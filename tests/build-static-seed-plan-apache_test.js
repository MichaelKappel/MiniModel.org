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
const { buildStaticSeedPlan } = require("../tools/build-static-seed-plan.js");
const { createOwnedOutput, removeOwnedOutput } = require("./browser-smoke.js");
const { descriptorBytes } = require("./helpers/static-seed-fixture.js");

const hash = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");

/** Run only the installed WSL tools; no package/image download or system configuration. */
function wsl(args, check = true) {
  const result = spawnSync("wsl.exe", ["-d", "OSM-Ubuntu", "--exec", ...args], {
    encoding: "utf8", timeout: 10000, maxBuffer: 128 * 1024, windowsHide: true,
    env: { ...process.env, PATH: path.join(process.env.SystemRoot, "System32"), NODE_V8_COVERAGE: "" },
  });
  if (check) assert.equal(result.status, 0, result.error?.message || result.stderr || result.stdout);
  return result;
}

/** Keep URL spelling intact, capture duplicate fields, and bound response bytes and duration. */
function request(port, target, method = "GET", headers = {}) {
  return new Promise((resolve, reject) => {
    const req = http.request({ host: "127.0.0.1", port, path: target, method,
      headers: { Origin: "https://tinyrustlm.com", "Accept-Encoding": "gzip, br", ...headers } }, (res) => {
      const chunks = [];
      let length = 0;
      res.on("data", (chunk) => {
        length += chunk.length;
        if (length > 64 * 1024) res.destroy(new Error("apache-response-size"));
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

/** Reserve a Windows loopback port briefly; an independent WSL collision remains a hard failure. */
async function freePort() {
  const server = http.createServer();
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return port;
}

/** WSL localhost forwarding can appear after Apache binds; wait only for the listener. */
async function waitForListener(port) {
  for (let attempt = 0; attempt < 40; attempt++) {
    try { await request(port, "/control/model.slm2", "HEAD"); return; }
    catch (error) {
      if (error.code !== "ECONNREFUSED" || attempt === 39) throw error;
      await delay(200);
    }
  }
}

/** Materialize small invalid-model fixtures solely to exercise HTTP; no ACG2 acceptance is claimed. */
function fixture(owned) {
  const descriptor = JSON.parse(descriptorBytes());
  descriptor.compositionId = `sha256:${hash("static seed transport fixture, not a model composition")}`;
  const directory = path.join(owned.directory, "members");
  fs.mkdirSync(directory);
  for (const member of descriptor.members) {
    const bytes = member.name === "composition.acg2" ? Buffer.alloc(896, 0x5a) :
      Buffer.from(`HTTP transport fixture ${member.name}\n`.repeat(80));
    member.bytes = bytes.length;
    member.sha256 = hash(bytes);
    fs.writeFileSync(path.join(directory, member.name), bytes, { flag: "wx" });
  }
  const plan = buildStaticSeedPlan(Buffer.from(`${JSON.stringify(descriptor)}\n`));
  fs.writeFileSync(path.join(directory, ".htaccess"), plan.configuration.text, { flag: "wx" });
  return plan;
}

/** Run an independent instance, preserving the installed server and its configuration. */
function configuration(linuxRoot, port, plan) {
  const modules = ["mpm_event", "authz_core", "alias", "dir", "env", "headers", "mime", "filter", "deflate", "rewrite"];
  return [
    `ServerRoot "${linuxRoot}"`, `DefaultRuntimeDir "${linuxRoot}"`,
    `PidFile "${linuxRoot}/apache.pid"`, `ErrorLog "${linuxRoot}/error.log"`,
    `Listen 127.0.0.1:${port}`, "ServerName 127.0.0.1", "ServerTokens Prod",
    ...plan.requiredServerConfiguration.directives,
    ...modules.map((name) => `LoadModule ${name}_module /usr/lib/apache2/modules/mod_${name}.so`),
    "User www-data", "Group www-data", "StartServers 1", "ServerLimit 1", "ThreadsPerChild 2",
    "MinSpareThreads 1", "MaxSpareThreads 2", "MaxRequestWorkers 2", "TypesConfig /etc/mime.types",
    `DocumentRoot "${linuxRoot}"`, "<Directory />", "AllowOverride None", "AllowOverrideList None",
    "Require all denied", "</Directory>",
    `Alias /${plan.remoteDirectory}/ "${linuxRoot}/members/"`,
    `Alias /invalid-directive/ "${linuxRoot}/invalid-directive/"`,
    `Alias /no-overrides/ "${linuxRoot}/no-overrides/"`,
    `Alias /control/ "${linuxRoot}/control/"`,
    `Header onsuccess set Access-Control-Allow-Origin "https://wrong.invalid"`,
    `Header always set Access-Control-Allow-Origin "https://also-wrong.invalid"`,
    `Header onsuccess set Access-Control-Allow-Credentials "true"`,
    `Header always set Access-Control-Allow-Credentials "true"`,
    `Header onsuccess set X-TinyRustLM-Piece-Set-Id "wrong"`,
    `Header always set X-TinyRustLM-Piece-Set-Id "also-wrong"`,
    "AddOutputFilterByType DEFLATE application/octet-stream",
    ...["members", "control", "invalid-directive", "no-overrides"].flatMap((name) => [
      `<Directory "${linuxRoot}/${name}">`, "Require all granted",
      `AllowOverride ${name === "no-overrides" ? "None" : "All"}`, "</Directory>",
    ]),
    "",
  ].join("\n");
}

/** Stop only the PID bound to our exact config and verify exit before deleting owned output. */
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

test("generated seed configuration serves exact members on installed Apache", async (t) => {
  const version = wsl(["/usr/sbin/apache2", "-v"]).stdout.trim();
  const owned = createOwnedOutput();
  let config;
  try {
    const plan = fixture(owned);
    const linuxRoot = wsl(["/usr/bin/wslpath", "-u", owned.directory]).stdout.trim();
    assert.match(linuxRoot, /^\/mnt\/e\/Source\/Rust\/MiniModel\.org\/artifacts\/tests\/minimodel-browser-[A-Za-z0-9]+$/);
    config = `${linuxRoot}/httpd.conf`;
    const port = await freePort();
    fs.writeFileSync(path.join(owned.directory, "httpd.conf"), configuration(linuxRoot, port, plan), { flag: "wx" });
    // Controls prove compression is available and ignored/missing directives do not pass admission.
    for (const name of ["control", "invalid-directive", "no-overrides"]) {
      fs.mkdirSync(path.join(owned.directory, name));
      fs.writeFileSync(path.join(owned.directory, name, "model.slm2"), "compressible control\n".repeat(300));
    }
    fs.writeFileSync(path.join(owned.directory, "control", ".htaccess"), "ForceType application/octet-stream\n");
    fs.writeFileSync(path.join(owned.directory, "no-overrides", ".htaccess"), plan.configuration.text);
    fs.writeFileSync(path.join(owned.directory, "invalid-directive", ".htaccess"), "MissingRequiredDirective on\n");
    wsl(["/usr/sbin/apache2", "-f", config, "-t"]);
    wsl(["/usr/sbin/apache2", "-f", config, "-k", "start"]);
    await waitForListener(port);

    for (const file of plan.files) await t.test(`GET and HEAD ${file.name}`, async () => {
      for (const method of ["GET", "HEAD"]) {
        const response = await request(port, `/${file.remotePath}`, method);
        assert.equal(response.status, 200, file.name);
        for (const [name, expected] of Object.entries(file.expectedHeaders)) {
          assert.equal(response.headers[name] ?? null, expected, `${method} ${name}`);
          assert.equal(response.rawHeaders.filter((_, i) => i % 2 === 0)
            .filter((key) => key.toLowerCase() === name).length, expected === null ? 0 : 1, `duplicate ${name}`);
        }
        assert.equal(response.headers["access-control-allow-credentials"], undefined);
        assert.equal(response.headers["cache-control"], "no-store, no-transform");
        for (const name of Object.keys(file.expectedHeaders).filter((key) => key.startsWith("x-"))) {
          assert.ok(response.headers["access-control-expose-headers"].toLowerCase().split(/,\s*/).includes(name));
        }
        if (method === "GET") { assert.equal(response.body.length, file.bytes); assert.equal(hash(response.body), file.sha256); }
        else assert.equal(response.body.length, 0);
      }
    });
    await t.test("reject invalid routes, queries and methods", async () => {
      const base = `/${plan.remoteDirectory}/`;
      for (const suffix of ["", ".htaccess", "unknown", "model.slm", "model.slm2/extra", "model.slm2?x=1"]) {
        assert.equal((await request(port, base + suffix)).status, 404, suffix);
      }
      for (const method of ["TRACE", "POST", "PUT", "DELETE", "OPTIONS"]) {
        const response = await request(port, base + "model.slm2", method);
        assert.equal(response.status, 405, method);
        // Core rejects TRACE before directory headers run; it must never advertise TRACE.
        if (method === "TRACE") assert.ok(!String(response.headers.allow).includes("TRACE"));
        // Apache's error handler can append an empty Allow field; compare the method list.
        else assert.deepEqual(response.headers.allow.split(",").map((value) => value.trim()).filter(Boolean), ["GET", "HEAD"]);
      }
    });
    await t.test("compression and override controls distinguish actual enforcement", async () => {
      assert.equal((await request(port, "/control/model.slm2")).headers["content-encoding"], "gzip");
      assert.equal((await request(port, "/invalid-directive/model.slm2")).status, 500);
      assert.notEqual((await request(port, "/no-overrides/model.slm2")).headers["x-tinyrustlm-piece-set-id"], plan.pieceSetId);
      const foreign = await request(port, `/${plan.files[0].remotePath}`, "GET", { Origin: "https://foreign.invalid" });
      assert.equal(foreign.headers["access-control-allow-origin"], "https://tinyrustlm.com");
    });
    t.diagnostic(JSON.stringify({ version, configurationSha256: plan.configuration.sha256,
      scope: "local HTTP on Apache; synthetic non-model bytes; no browser/TLS/LiteSpeed/publication claim" }));
  } finally {
    if (config) await stopApache(owned, config);
    const log = path.join(owned.directory, "error.log");
    if (fs.existsSync(log)) t.diagnostic(fs.readFileSync(log, "utf8").slice(-4096));
    t.diagnostic(JSON.stringify({ ownedOutputRemoved: removeOwnedOutput(owned) }));
  }
});
