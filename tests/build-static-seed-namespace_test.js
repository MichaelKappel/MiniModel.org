"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");
const { buildStaticSeedNamespace } = require("../tools/build-static-seed-namespace.js");
const { buildStaticSeedPlan } = require("../tools/build-static-seed-plan.js");
const { descriptorBytes } = require("./helpers/static-seed-fixture.js");

function registry(descriptor = null) {
  return Buffer.from(`${JSON.stringify({ schema: "minimodel.static-seed-namespace/1",
    descriptorBase64: descriptor === null ? null : descriptor.toString("base64") })}\n`);
}

test("empty namespace permits no seed path and claims no publication", () => {
  const bytes = registry();
  const result = buildStaticSeedNamespace(bytes);
  assert.equal(result.publicationAllowed, false);
  assert.equal(result.compositionId, null);
  assert.equal(result.plan, null);
  assert.deepEqual(result.files, []);
  assert.equal(result.registrySha256, crypto.createHash("sha256").update(bytes).digest("hex"));
});

test("one descriptor admits exactly its six members and generated configuration", () => {
  const descriptor = descriptorBytes();
  const plan = buildStaticSeedPlan(descriptor);
  const result = buildStaticSeedNamespace(registry(descriptor));
  assert.deepEqual(result.plan, plan);
  assert.equal(result.publicationAllowed, false);
  assert.deepEqual(result.files, [...plan.files, plan.configuration].map((file) => ({
    path: file.remotePath, bytes: file.bytes, sha256: file.sha256,
  })));
  assert.equal(result.files.length, 7);
  assert.equal(new Set(result.files.map((file) => file.path)).size, 7);
  assert.ok(result.files.every((file) => file.path.startsWith(`${plan.remoteDirectory}/`)));
});

test("namespace rejects malformed, noncanonical, oversized and extra authority fields", () => {
  for (const bytes of [null, "{}", Buffer.alloc(0), Buffer.alloc(16_385)]) {
    assert.throws(() => buildStaticSeedNamespace(bytes), /static-seed-namespace-size/);
  }
  for (const bytes of [Buffer.from("{"), Buffer.from([0xff])]) {
    assert.throws(() => buildStaticSeedNamespace(bytes), /static-seed-namespace-json/);
  }
  for (const value of [null, [], {}, { schema: "old", descriptorBase64: null },
    { schema: "minimodel.static-seed-namespace/1", descriptorBase64: 1 },
    { schema: "minimodel.static-seed-namespace/1", descriptorBase64: null, qualified: true }]) {
    assert.throws(() => buildStaticSeedNamespace(Buffer.from(`${JSON.stringify(value)}\n`)),
      /static-seed-namespace-canonical/);
  }
  assert.throws(() => buildStaticSeedNamespace(Buffer.from(registry().toString().trim())), /canonical/);
  assert.throws(() => buildStaticSeedNamespace(Buffer.from(
    '{"schema":"minimodel.static-seed-namespace/1","descriptorBase64":null,"descriptorBase64":null}\n')), /canonical/);
});

test("namespace uses the existing exact descriptor parser and rejects alternate base64", () => {
  for (const descriptorBase64 of ["", "?", "Zg", "Zg==\n"]) {
    const bytes = Buffer.from(`${JSON.stringify({ schema: "minimodel.static-seed-namespace/1", descriptorBase64 })}\n`);
    assert.throws(() => buildStaticSeedNamespace(bytes), /static-seed-namespace-base64/);
  }
  assert.throws(() => buildStaticSeedNamespace(registry(Buffer.from("{}"))), /static-seed-descriptor-/);
  const invalid = JSON.parse(descriptorBytes());
  invalid.members[0].name = "../model.slm2";
  assert.throws(() => buildStaticSeedNamespace(registry(Buffer.from(`${JSON.stringify(invalid)}\n`))), /static-seed-descriptor-/);
});

test("CLI reads the checked-in empty registry and rejects any input path override", () => {
  const tool = path.resolve(__dirname, "../tools/build-static-seed-namespace.js");
  const env = { ...process.env, NODE_V8_COVERAGE: "" };
  for (const args of [[], ["elsewhere.json"]]) {
    const result = spawnSync(process.execPath, [tool, ...args], {
      encoding: "utf8", windowsHide: true, timeout: 10_000, maxBuffer: 65_536, env,
    });
    assert.ifError(result.error);
    if (args.length === 0) {
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(JSON.parse(result.stdout).files, []);
    } else {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /usage:/);
    }
  }
});
