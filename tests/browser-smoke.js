"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const http = require("node:http");
const path = require("node:path");
const { once } = require("node:events");

const root = path.resolve(__dirname, "..");
const canonicalRoot = "E:\\Source\\Rust\\MiniModel.org";
const outputParent = path.join(root, "artifacts", "tests");
const maximumSourceBytes = 1024 * 1024;
const sources = Object.freeze({
  "/": ["index.html", "text/html"],
  "/styles.css": ["styles.css", "text/css"],
  "/catalog.js": ["catalog.js", "text/javascript"],
  "/catalog/compositions.v1.json": ["catalog/compositions.v1.json", "application/json"],
  "/catalog/peer-announcements.v1.json": ["catalog/peer-announcements.v1.json", "application/json"],
  "/updates/": ["updates/index.html", "text/html"],
});
const sha256 = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");
let running = false;

// No output or browser process can be redirected to another source tree.
function assertOrdinaryAncestors(target) {
  for (let current = target; ; current = path.dirname(current)) {
    const stat = fs.lstatSync(current, { throwIfNoEntry: false });
    assert.ok(!stat || !stat.isSymbolicLink(), "Browser output has a linked ancestor");
    if (path.dirname(current) === current) break;
  }
}

function assertCanonicalRoot() {
  assert.equal(process.platform, "win32", "This canonical browser check runs on Windows");
  assert.equal(root.toLowerCase(), canonicalRoot.toLowerCase());
  assertOrdinaryAncestors(root);
}

function createOwnedOutput() {
  assertCanonicalRoot();
  assertOrdinaryAncestors(outputParent);
  fs.mkdirSync(outputParent, { recursive: true });
  assertOrdinaryAncestors(outputParent);
  const directory = fs.mkdtempSync(path.join(outputParent, "minimodel-browser-"));
  const marker = path.join(directory, ".operation-owner");
  const bytes = Buffer.from(crypto.randomUUID() + "\n");
  fs.writeFileSync(marker, bytes, { flag: "wx" });
  return Object.freeze({ directory, marker, bytes, identity: fs.lstatSync(directory, { bigint: true }) });
}

// Inspect the whole bounded owned output tree before recursive deletion, never follow links.
function removeOwnedOutput(owned) {
  assertCanonicalRoot();
  assert.equal(path.dirname(owned.directory), outputParent, "Browser output escaped its canonical parent");
  assert.match(path.basename(owned.directory), /^minimodel-browser-[A-Za-z0-9]+$/);
  assertOrdinaryAncestors(owned.directory);
  const stat = fs.lstatSync(owned.directory, { bigint: true });
  assert.ok(stat.isDirectory() && stat.ino === owned.identity.ino && stat.dev === owned.identity.dev, "Browser output identity changed");
  assert.equal(owned.marker, path.join(owned.directory, ".operation-owner"));
  const marker = fs.lstatSync(owned.marker);
  assert.ok(marker.isFile() && !marker.isSymbolicLink() && marker.nlink === 1 && marker.size === owned.bytes.length, "Browser output owner changed");
  assert.deepEqual(fs.readFileSync(owned.marker), owned.bytes, "Browser output owner changed");
  const pending = [[owned.directory, 0]];
  let files = 0;
  let bytes = 0;
  while (pending.length) {
    const [directory, depth] = pending.pop();
    assert.ok(depth <= 32, "Browser output depth exceeded");
    for (const name of fs.readdirSync(directory)) {
      assert.ok(++files <= 8192, "Browser output entry bound exceeded");
      const target = path.join(directory, name);
      const entry = fs.lstatSync(target);
      assert.ok(!entry.isSymbolicLink(), "Browser output contains a link");
      if (entry.isDirectory()) pending.push([target, depth + 1]);
      else {
        assert.ok(entry.isFile() && entry.nlink === 1, "Browser output contains a special or linked file");
        bytes += entry.size;
        assert.ok(bytes <= 256 * 1024 * 1024, "Browser output byte bound exceeded");
      }
    }
  }
  fs.rmSync(owned.directory, { recursive: true, force: false, maxRetries: 3, retryDelay: 100 });
  assert.equal(fs.existsSync(owned.directory), false, "Browser output cleanup was not proved");
  return { files, bytes };
}

// Serve only six explicit canonical inputs, held in bounded memory for one browser run.
function readSourceInputs() {
  assertCanonicalRoot();
  return Object.fromEntries(Object.entries(sources).map(([route, [relative, type]]) => {
    const target = path.join(root, relative);
    assertOrdinaryAncestors(target);
    const fd = fs.openSync(target, "r");
    let bytes;
    try {
      const stat = fs.fstatSync(fd);
      assert.ok(stat.isFile() && stat.size <= maximumSourceBytes);
      bytes = Buffer.alloc(stat.size);
      let offset = 0;
      while (offset < bytes.length) {
        const count = fs.readSync(fd, bytes, offset, bytes.length - offset, null);
        assert.ok(count > 0, "Browser input shrank during its bounded read");
        offset += count;
      }
      assert.equal(fs.readSync(fd, Buffer.alloc(1), 0, 1, null), 0, "Browser input grew during its bounded read");
    } finally { fs.closeSync(fd); }
    return [route, { relative, type, bytes, sha256: sha256(bytes) }];
  }));
}

function createPreviewServer(inputs) {
  const state = { rejectMetadata: false };
  const server = http.createServer((request, response) => {
    const host = `127.0.0.1:${server.address().port}`;
    if (request.headers.host !== host) { response.writeHead(403).end(); return; }
    if (!["GET", "HEAD"].includes(request.method)) { response.writeHead(405, { Allow: "GET, HEAD" }).end(); return; }
    const target = (request.url || "").split("?")[0];
    const input = Object.hasOwn(inputs, target) ? inputs[target] : null;
    if (!input) { response.writeHead(404).end(); return; }
    if (state.rejectMetadata && target.startsWith("/catalog/")) { response.writeHead(503).end(); return; }
    response.writeHead(200, { "Content-Type": `${input.type}; charset=utf-8`, "Content-Length": input.bytes.length,
      "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" });
    response.end(request.method === "HEAD" ? undefined : input.bytes);
  });
  server.requestTimeout = 5000;
  server.headersTimeout = 5000;
  return { server, state };
}

async function inspectViewport(page, label, expectedState) {
  await page.waitForFunction(() => ["ready", "error"].includes(document.getElementById("modelCatalogStatus")?.dataset.state), null, { timeout: 15000 });
  const result = await page.evaluate(() => ({
    width: innerWidth, height: innerHeight,
    documentWidth: document.documentElement.scrollWidth, bodyWidth: document.body.scrollWidth,
    catalogTop: document.getElementById("models").getBoundingClientRect().top,
    heading: (() => { const box = document.querySelector(".hero h1").getBoundingClientRect(); return { left: box.left, right: box.right }; })(),
    state: document.getElementById("modelCatalogStatus").dataset.state,
    status: document.getElementById("modelCatalogStatus").textContent,
    models: document.querySelectorAll("#modelCatalogList .catalog-model").length,
    actions: document.querySelectorAll("#modelCatalogList a, #modelCatalogList button").length,
    list: document.getElementById("modelCatalogList").textContent.trim(),
    totals: document.querySelector(".catalog-totals").textContent.trim().replace(/\s+/g, " "),
    searchDisabled: document.getElementById("modelCatalogSearch").disabled,
    filterDisabled: document.getElementById("modelCatalogAvailability").disabled,
    filter: document.getElementById("modelCatalogAvailability").value,
    footer: document.querySelector("footer").textContent.trim().replace(/\s+/g, " "),
  }));
  assert.equal(result.state, expectedState, label);
  assert.equal(result.models, 0, label);
  assert.equal(result.actions, 0, label);
  assert.ok(result.documentWidth <= result.width + 1 && result.bodyWidth <= result.width + 1, `${label}: horizontal overflow`);
  assert.ok(result.catalogTop < result.height, `${label}: catalog is below the initial viewport`);
  assert.ok(result.heading.left >= 0 && result.heading.right <= result.width + 1, `${label}: heading is off canvas`);
  assert.match(result.footer, /Version 1\.1\.2/);
  assert.doesNotMatch(result.footer, /Last published|Prepared 2026/);
  if (expectedState === "error") {
    assert.match(result.list, /catalog is unavailable/);
    assert.doesNotMatch(result.list + result.totals, /Loading|Checking/);
    assert.equal(result.searchDisabled, true);
    assert.equal(result.filterDisabled, true);
  } else {
    assert.match(result.list, /No SLM2 composition is qualified yet/);
    assert.match(result.totals, /0 qualified compositions/);
    assert.equal(result.searchDisabled, false);
    assert.equal(result.filterDisabled, false);
    assert.equal(result.filter, "all");
  }
  const screenshot = await page.screenshot({ animations: "disabled" });
  return { label, ...result, screenshot: { bytes: screenshot.length, sha256: sha256(screenshot) } };
}

async function runBrowserSmoke() {
  assert.equal(running, false, "Browser smoke already owns this process's temporary environment");
  running = true;
  let owned, context, server;
  let browserClosed = true;
  const previous = Object.fromEntries(["TMP", "TEMP", "TMPDIR"].map((name) => [name, process.env[name]]));
  const report = { schema: "minimodel.browser_smoke.v2", started_utc: new Date().toISOString(), observations: [] };
  try {
    const inputs = readSourceInputs();
    // This is deliberately a historical empty-catalog UI fixture, never model qualification.
    const catalog = JSON.parse(inputs["/catalog/compositions.v1.json"].bytes);
    assert.deepEqual(catalog.entries, []);
    assert.ok(Date.parse(catalog.valid_until_utc) < Date.now(), "Current expired-catalog case needs review because source authority changed");
    report.source_inputs = Object.values(inputs).map(({ relative, bytes, sha256 }) => ({ path: relative, bytes: bytes.length, sha256 }));
    report.historical_fixture_time = catalog.generated_at_utc;
    owned = createOwnedOutput();
    const temporary = path.join(owned.directory, "temporary");
    fs.mkdirSync(temporary);
    for (const name of Object.keys(previous)) process.env[name] = temporary;
    const playwrightRoot = path.join(path.dirname(root), "TinyRustLM.com", "tinyrustlm", "node_modules", "playwright-core");
    assertOrdinaryAncestors(playwrightRoot);
    const { chromium } = require(playwrightRoot);
    report.playwright_version = require(path.join(playwrightRoot, "package.json")).version;
    const preview = createPreviewServer(inputs);
    server = preview.server;
    server.listen(0, "127.0.0.1");
    await once(server, "listening");
    const origin = `http://127.0.0.1:${server.address().port}`;
    const childEnv = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.toUpperCase().startsWith("NODE_")));
    for (const [label, width, height] of [["desktop", 1440, 1000], ["mobile", 390, 844]]) {
      // A fresh browser prevents the historical fixture clock leaking into the next current-time case.
      context = await chromium.launchPersistentContext(path.join(owned.directory, `profile-${label}`), {
        channel: "msedge", headless: true, timeout: 15000, env: childEnv,
        downloadsPath: temporary, artifactsDir: temporary, acceptDownloads: false,
      });
      browserClosed = false;
      report.browser_version = context.browser().version();
      await context.route("**/*", (route) => new URL(route.request().url()).origin === origin ? route.continue() : route.abort());
      const page = await context.newPage();
      const errors = [];
      page.on("pageerror", (error) => errors.push(error.message));
      await page.setViewportSize({ width, height });
      await page.goto(origin, { waitUntil: "load", timeout: 15000 });
      report.observations.push(await inspectViewport(page, `${label}:current-expired-source`, "error"));
      await page.clock.setFixedTime(new Date(catalog.generated_at_utc));
      await page.goto(origin, { waitUntil: "load", timeout: 15000 });
      report.observations.push(await inspectViewport(page, `${label}:historical-empty-fixture`, "ready"));
      preview.state.rejectMetadata = true;
      const failed = await page.evaluate(() => window.MiniModelCatalog.loadCatalog(document).then(() => false, () => true));
      assert.equal(failed, true);
      report.observations.push(await inspectViewport(page, `${label}:failed-reload-fixture`, "error"));
      preview.state.rejectMetadata = false;
      await page.evaluate(() => window.MiniModelCatalog.loadCatalog(document));
      report.observations.push(await inspectViewport(page, `${label}:recovered-empty-fixture`, "ready"));
      assert.deepEqual(errors, [], `${label}: unhandled page errors`);
      await page.close();
      await context.close();
      browserClosed = true;
      context = null;
    }
    return report;
  } finally {
    try {
      if (context) { await context.close(); browserClosed = true; }
    } finally {
      try {
        if (server?.listening) { server.closeAllConnections(); await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve())); }
        if (owned) {
          assert.ok(browserClosed, `Browser closure is unproved; preserve owned output at ${owned.directory}`);
          report.cleaned_output = removeOwnedOutput(owned);
          report.output_absent = true;
        }
      } finally {
        for (const [name, value] of Object.entries(previous)) {
          if (value === undefined) delete process.env[name]; else process.env[name] = value;
        }
        running = false;
      }
    }
  }
}

module.exports = Object.freeze({ createOwnedOutput, removeOwnedOutput, readSourceInputs, createPreviewServer, runBrowserSmoke });
if (require.main === module) {
  if (process.argv.length !== 2) {
    process.stderr.write("Usage: node tests/browser-smoke.js (canonical local source only)\n");
    process.exitCode = 1;
  } else {
    runBrowserSmoke().then((report) => process.stdout.write(JSON.stringify(report, null, 2) + "\n"), (error) => {
      process.stderr.write((error.stack || String(error)) + "\n");
      process.exitCode = 1;
    });
  }
}
