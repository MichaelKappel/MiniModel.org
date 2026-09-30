"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const http = require("node:http");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { once } = require("node:events");
const { setTimeout: delay } = require("node:timers/promises");
const test = require("node:test");
const { createOwnedOutput, removeOwnedOutput } = require("./browser-smoke.js");

const canonicalRewrite = path.resolve(__dirname, "../deploy/host-renewal-catalog.htaccess");
const canonicalHandlerRules = path.resolve(__dirname, "../deploy/host-renewal/.htaccess");
const handlerPath = "/.minimodel-metadata-renewal.v1/metadata.php";
const routes = ["compositions.v1.json", "peer-announcements.v1.json"];
const sha256 = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");

/** Installed tools only; the child never inherits Node hooks or a foreign test PATH. */
function wsl(args, check = true) {
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.toUpperCase().startsWith("NODE_")));
  env.PATH = path.join(process.env.SystemRoot, "System32");
  const result = spawnSync("wsl.exe", ["-d", "OSM-Ubuntu", "--exec", ...args], {
    encoding: "utf8", timeout: 10000, maxBuffer: 128 * 1024, windowsHide: true, env,
  });
  if (check) assert.equal(result.status, 0, result.error?.message || result.stderr || result.stdout);
  return result;
}

/** Preserve raw spelling and duplicate headers; never transmit fixture data off loopback. */
function request(port, target, method = "GET") {
  return new Promise((resolve, reject) => {
    const req = http.request({ host: "127.0.0.1", port, path: target, method,
      headers: { Origin: "https://tinyrustlm.com" } }, (res) => {
      const chunks = [];
      let bytes = 0;
      res.on("data", (chunk) => {
        bytes += chunk.length;
        if (bytes > 64 * 1024) res.destroy(new Error("apache-response-size"));
        else chunks.push(chunk);
      });
      res.on("error", reject);
      res.on("end", () => resolve({ status: res.statusCode, headers: res.headers,
        rawHeaders: res.rawHeaders, body: Buffer.concat(chunks) }));
    });
    const timer = setTimeout(() => req.destroy(new Error("apache-response-timeout")), 5000);
    req.on("close", () => clearTimeout(timer));
    req.on("error", reject);
    req.end();
  });
}

function headerValues(response, name) {
  const result = [];
  for (let index = 0; index < response.rawHeaders.length; index += 2) {
    if (response.rawHeaders[index].toLowerCase() === name) result.push(response.rawHeaders[index + 1]);
  }
  return result;
}

async function freePort() {
  const server = http.createServer();
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return port;
}

async function waitForListener(port) {
  for (let attempt = 0; attempt < 40; attempt++) {
    try { await request(port, "/catalog/unrelated.json", "HEAD"); return; }
    catch (error) {
      if (error.code !== "ECONNREFUSED" || attempt === 39) throw error;
      await delay(200);
    }
  }
}

/** This generated non-PHP CGI fixture exposes Apache routing, not renewal implementation. */
function fixture(directory) {
  const catalog = path.join(directory, "catalog");
  const handler = path.join(directory, handlerPath.slice(1));
  fs.mkdirSync(catalog);
  fs.mkdirSync(path.dirname(handler));
  fs.writeFileSync(path.join(path.dirname(handler), "renewal-core.php"), "denied synthetic core sentinel\n", { flag: "wx" });
  fs.writeFileSync(path.join(catalog, "unrelated.json"), '{"unrelated":true}\n', { flag: "wx" });
  for (const route of routes) fs.writeFileSync(path.join(catalog, route), '{"staleStatic":true}\n', { flag: "wx" });
  for (const route of ["compositions.v1.json.extra"]) {
    fs.writeFileSync(path.join(catalog, route), '{"nearMatch":true}\n', { flag: "wx" });
  }
  fs.writeFileSync(handler, [
    "#!/usr/bin/python3", "import json, os", "print('Content-Type: application/json')",
    "print('X-Synthetic-Handler: apache-routing-only')",
    "print('X-Synthetic-Query: ' + os.environ.get('QUERY_STRING', ''))",
    "for name in ['Access-Control-Allow-Origin', 'Access-Control-Allow-Methods', 'Access-Control-Allow-Credentials', 'Cache-Control', 'X-Content-Type-Options']:",
    "    print(name + ': wrong-always-one')", "    print(name + ': wrong-always-two')",
    "print()", "print(json.dumps({'method': os.environ.get('REQUEST_METHOD'), 'requestUri': os.environ.get('REQUEST_URI'), 'query': os.environ.get('QUERY_STRING'), 'scriptName': os.environ.get('SCRIPT_NAME')}))", "",
  ].join("\n"), { flag: "wx" });
}

function configuration(linuxRoot, linuxRewrite, linuxHandlerRules, port) {
  const modules = ["mpm_prefork", "authz_core", "alias", "dir", "headers", "mime", "rewrite", "cgi"];
  return [
    `ServerRoot "${linuxRoot}"`, `DefaultRuntimeDir "${linuxRoot}"`,
    `PidFile "${linuxRoot}/apache.pid"`, `ErrorLog "${linuxRoot}/error.log"`,
    `Listen 127.0.0.1:${port}`, "ServerName 127.0.0.1", "ServerTokens Prod",
    ...modules.map((name) => `LoadModule ${name}_module /usr/lib/apache2/modules/mod_${name}.so`),
    "User www-data", "Group www-data", "StartServers 1", "ServerLimit 2", "MinSpareServers 1", "MaxSpareServers 2", "MaxRequestWorkers 2",
    "TypesConfig /etc/mime.types", `DocumentRoot "${linuxRoot}"`,
    "<Directory />", "Require all denied", "AllowOverride None", "</Directory>",
    // Deliberate bad onsuccess values complement the CGI handler's duplicate always values.
    ...["Access-Control-Allow-Origin", "Access-Control-Allow-Methods", "Access-Control-Allow-Credentials", "Cache-Control", "X-Content-Type-Options"]
      .map((name) => `Header onsuccess set ${name} "wrong-onsuccess"`),
    `<Directory "${linuxRoot}/catalog">`, "Require all granted", `Include "${linuxRewrite}"`, "</Directory>",
    `<Directory "${linuxRoot}/.minimodel-metadata-renewal.v1">`, "Require all granted", "Options +ExecCGI", "AddHandler cgi-script .php",
    `Include "${linuxHandlerRules}"`, "</Directory>", "",
  ].join("\n");
}

/** Stop only the PID that still names this operation's exact Apache configuration. */
async function stopApache(owned, config) {
  const pidFile = path.join(owned.directory, "apache.pid");
  if (!fs.existsSync(pidFile)) return;
  const pid = fs.readFileSync(pidFile, "utf8").trim();
  assert.match(pid, /^[1-9][0-9]{0,8}$/);
  const command = wsl(["/usr/bin/cat", `/proc/${pid}/cmdline`]).stdout.split("\0");
  assert.equal(command[0], "/usr/sbin/apache2");
  assert.equal(command[command.indexOf("-f") + 1], config);
  wsl(["/usr/sbin/apache2", "-f", config, "-k", "stop"]);
  for (let attempt = 0; attempt < 40; attempt++) {
    const exists = wsl(["/usr/bin/test", "-e", `/proc/${pid}`], false);
    if (exists.status === 1) return;
    assert.equal(exists.status, 0, "Unable to establish owned Apache exit");
    await delay(100);
  }
  throw new Error("Owned Apache did not exit; output retained for review");
}

test("canonical renewal rewrite on installed Apache preserves exact routes and response policy", async (t) => {
  const version = wsl(["/usr/sbin/apache2", "-v"]).stdout.trim();
  const rewriteIdentity = sha256(fs.readFileSync(canonicalRewrite));
  const handlerRulesIdentity = sha256(fs.readFileSync(canonicalHandlerRules));
  const owned = createOwnedOutput();
  let config;
  try {
    fixture(owned.directory);
    const linuxRoot = wsl(["/usr/bin/wslpath", "-u", owned.directory]).stdout.trim();
    const linuxRewrite = wsl(["/usr/bin/wslpath", "-u", canonicalRewrite]).stdout.trim();
    const linuxHandlerRules = wsl(["/usr/bin/wslpath", "-u", canonicalHandlerRules]).stdout.trim();
    assert.match(linuxRoot, /^\/mnt\/e\/Source\/Rust\/MiniModel\.org\/artifacts\/tests\/minimodel-browser-[A-Za-z0-9]+$/);
    assert.equal(linuxRewrite, "/mnt/e/Source/Rust/MiniModel.org/deploy/host-renewal-catalog.htaccess");
    assert.equal(linuxHandlerRules, "/mnt/e/Source/Rust/MiniModel.org/deploy/host-renewal/.htaccess");
    wsl(["/usr/bin/chmod", "755", `${linuxRoot}${handlerPath}`]);
    const port = await freePort();
    config = `${linuxRoot}/httpd.conf`;
    fs.writeFileSync(path.join(owned.directory, "httpd.conf"), configuration(linuxRoot, linuxRewrite, linuxHandlerRules, port), { flag: "wx" });
    wsl(["/usr/sbin/apache2", "-f", config, "-t"]);
    wsl(["/usr/sbin/apache2", "-f", config, "-k", "start"]);
    await waitForListener(port);
    for (const route of routes) await t.test(`exact ${route} GET/HEAD and query forwarding`, async () => {
      for (const query of ["", "?diagnostic=one%20two&duplicate=1&duplicate=2"]) {
        for (const method of ["GET", "HEAD"]) {
          const target = `/catalog/${route}${query}`;
          const response = await request(port, target, method);
          assert.equal(response.status, 200);
          assert.equal(response.headers["x-synthetic-handler"], "apache-routing-only");
          assert.equal(response.headers["x-synthetic-query"] || "", query.slice(1));
          for (const [name, value] of Object.entries({
            "access-control-allow-origin": "https://tinyrustlm.com", "access-control-allow-methods": "GET, HEAD",
            "cache-control": "no-store, no-transform", "x-content-type-options": "nosniff",
          })) assert.deepEqual(headerValues(response, name), [value], `${method} ${target} ${name}`);
          assert.deepEqual(headerValues(response, "access-control-allow-credentials"), []);
          assert.deepEqual(headerValues(response, "content-type"), ["application/json"]);
          if (method === "HEAD") assert.equal(response.body.length, 0);
          else assert.deepEqual(JSON.parse(response.body), { method, requestUri: target, query: query.slice(1), scriptName: handlerPath });
        }
      }
    });
    await t.test("unrelated catalog resources retain their existing wildcard policy", async () => {
      for (const route of ["unrelated.json", "compositions.v1.json.extra", "Compositions.v1.json"]) {
        for (const method of ["GET", "HEAD"]) {
          const response = await request(port, `/catalog/${route}`, method);
          assert.equal(response.status, 200);
          assert.equal(response.headers["x-synthetic-handler"], undefined);
          assert.deepEqual(headerValues(response, "access-control-allow-origin"), ["*"]);
          assert.deepEqual(headerValues(response, "access-control-allow-methods"), ["GET, OPTIONS"]);
          if (method === "HEAD") assert.equal(response.body.length, 0);
          else assert.deepEqual(JSON.parse(response.body), route === "unrelated.json" ? { unrelated: true }
            : route === "Compositions.v1.json" ? { staleStatic: true } : { nearMatch: true });
        }
      }
      assert.equal((await request(port, "/catalog/compositions.v1.json/extra")).status, 404);
      assert.equal((await request(port, "/catalog/missing.json")).status, 404);
    });
    await t.test("direct handler access does not acquire original-route policy and core remains denied", async () => {
      const direct = await request(port, `${handlerPath}?direct=1`);
      assert.equal(direct.status, 200, "Synthetic CGI fixture responds without executing PHP policy");
      assert.equal(direct.headers["x-synthetic-handler"], "apache-routing-only");
      assert.deepEqual(headerValues(direct, "access-control-allow-origin"),
        ["wrong-always-one, wrong-always-two", "wrong-onsuccess"]);
      assert.equal((await request(port, "/.minimodel-metadata-renewal.v1/renewal-core.php")).status, 403);
    });
    assert.equal(sha256(fs.readFileSync(canonicalRewrite)), rewriteIdentity, "Canonical rewrite changed during the check");
    assert.equal(sha256(fs.readFileSync(canonicalHandlerRules)), handlerRulesIdentity, "Canonical handler rules changed during the check");
    t.diagnostic(JSON.stringify({ version, rewriteSha256: rewriteIdentity, handlerRulesSha256: handlerRulesIdentity,
      scope: "isolated Apache routing/header configuration with synthetic Python CGI; no PHP, live host, catalog-signing, model, TLS or publication claim" }));
  } finally {
    if (config) await stopApache(owned, config);
    const log = path.join(owned.directory, "error.log");
    if (fs.existsSync(log)) t.diagnostic(fs.readFileSync(log, "utf8").slice(-4096));
    t.diagnostic(JSON.stringify({ ownedOutputRemoved: removeOwnedOutput(owned) }));
  }
});
