"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const http = require("node:http");
const path = require("node:path");
const { once } = require("node:events");
const { spawnSync } = require("node:child_process");
const { test, mock } = require("node:test");
const smoke = require("./browser-smoke.js");
const root = path.resolve(__dirname, "..");

test("browser output ownership rejects another root or changed marker before removing anything", () => {
  const owned = smoke.createOwnedOutput();
  try {
    fs.writeFileSync(path.join(owned.directory, "probe.txt"), "operation-owned non-source output");
    assert.throws(() => smoke.removeOwnedOutput({ ...owned, directory: root }), /canonical parent/);
    fs.writeFileSync(owned.marker, "x".repeat(owned.bytes.length));
    assert.throws(() => smoke.removeOwnedOutput(owned), /owner changed/);
    assert.ok(fs.existsSync(path.join(owned.directory, "probe.txt")));
    fs.writeFileSync(owned.marker, owned.bytes);
    const lstat = fs.lstatSync.bind(fs);
    const probe = path.join(owned.directory, "probe.txt");
    const mocked = mock.method(fs, "lstatSync", (target, ...args) => target === probe ? { isSymbolicLink: () => true } : lstat(target, ...args));
    try { assert.throws(() => smoke.removeOwnedOutput(owned), /contains a link/); }
    finally { mocked.mock.restore(); }
  } finally { smoke.removeOwnedOutput(owned); }
  assert.equal(fs.existsSync(owned.directory), false);
});

test("loopback preview serves exact canonical public bytes and excludes private source and foreign hosts", async () => {
  const inputs = smoke.readSourceInputs();
  const { server, state } = smoke.createPreviewServer(inputs);
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const port = server.address().port;
  const request = (target, method = "GET", host = `127.0.0.1:${port}`) => new Promise((resolve, reject) => {
    const req = http.request({ hostname: "127.0.0.1", port, path: target, method, headers: { host } }, (response) => {
      const chunks = [];
      response.on("data", (data) => chunks.push(data));
      response.on("end", () => resolve({ status: response.statusCode, bytes: Buffer.concat(chunks) }));
    });
    req.on("error", reject);
    req.end();
  });
  try {
    for (const [route, value] of Object.entries(inputs)) {
      const result = await request(route);
      assert.equal(result.status, 200);
      assert.deepEqual(result.bytes, fs.readFileSync(path.join(root, value.relative)));
    }
    assert.equal((await request("/", "HEAD")).bytes.length, 0);
    for (const route of ["/.git/HEAD", "/.uai/taboo.uai", "/tests/browser-smoke.js", "/deploy/publication-authority.v1.json", "/../index.html", "/%2e%2e/index.html", "//index.html", "/index.html::$DATA"]) {
      assert.equal((await request(route)).status, 404, route);
    }
    assert.equal((await request("/", "POST")).status, 405);
    assert.equal((await request("/", "GET", "attacker.example")).status, 403);
    state.rejectMetadata = true;
    assert.equal((await request("/catalog/compositions.v1.json")).status, 503);
    assert.equal((await request("/styles.css")).status, 200);
  } finally { await new Promise((resolve) => server.close(resolve)); }
});

test("CLI rejects external targets and arbitrary output paths without starting a browser", () => {
  for (const argument of ["https://minimodel.org/", "http://127.0.0.1:8191/", "E:\\unexpected-output"]) {
    const result = spawnSync(process.execPath, [path.join(__dirname, "browser-smoke.js"), argument], { encoding: "utf8", timeout: 5000, windowsHide: true });
    assert.equal(result.status, 1);
    assert.equal(result.stdout, "");
    assert.match(result.stderr, /canonical local source only/);
  }
});

test("real Edge verifies current expiry and explicitly historical empty/reload/recovery states with complete cleanup", async () => {
  const parent = path.join(root, "artifacts", "tests");
  const environment = Object.fromEntries(["TMP", "TEMP", "TMPDIR"].map((name) => [name, process.env[name]]));
  const report = await smoke.runBrowserSmoke();
  assert.equal(report.observations.length, 8);
  assert.equal(report.output_absent, true);
  assert.equal(path.dirname(report.operation_directory), parent);
  assert.equal(fs.existsSync(report.operation_directory), false);
  assert.deepEqual(Object.fromEntries(Object.keys(environment).map((name) => [name, process.env[name]])), environment);
  process.stdout.write(`Browser verification: ${JSON.stringify(report)}\n`);
});
