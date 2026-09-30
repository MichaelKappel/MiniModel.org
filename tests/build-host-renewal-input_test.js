"use strict";
const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");
const { buildHostRenewalInput, parseEnvelope } = require("../tools/build-host-renewal-input.js");
const { createAdmittedSeedFixture } = require("./helpers/static-seed-publication-fixture.js");
const api = require("../../TinyRustLM.com/tinyrustlm/app/minimodel-catalog-v1.js");

const canon = (value) => Buffer.from(`${JSON.stringify(value)}\n`);
const sha = (value) => crypto.createHash("sha256").update(value).digest("hex");
const owner = () => ({ site: "minimodel.org", release_id: "synthetic-test-never-live", source_commit: "a".repeat(40),
  publisher_commit: "b".repeat(40), manifest_sha256: "c".repeat(64), authority_sha256: "d".repeat(64) });
function fixture() {
  const { inputs, privateKey } = createAdmittedSeedFixture({ renewal: true });
  return { inputs: { ...inputs, license: Buffer.from("Synthetic fixture license only; never a model license.\n") },
    releaseOwner: owner(), privateKey };
}
function change(f, name, edit) { const value = JSON.parse(f.inputs[name]); edit(value); f.inputs[name] = canon(value); }
function sign(f) { change(f, "peerFeed", (value) => {
  value.announcements[0].signature = crypto.sign(null, api.peerAnnouncementSigningBytes(value.announcements[0]), f.privateKey).toString("base64url");
}); }
const encode = (f) => canon({ inputs: Object.fromEntries(Object.entries(f.inputs).map(([name, bytes]) => [name, bytes.toString("base64")])),
  releaseOwner: f.releaseOwner });

test("renewal input uses actual admission and binds canonical pair to exactly nine ordered references", async () => {
  const f = fixture(), result = await buildHostRenewalInput(f.inputs, f.releaseOwner);
  assert.equal(result.publicationAllowed, false);
  assert.equal(result.status, "metadata-admitted-host-and-key-unverified");
  assert.match(result.qualityScope, /^test-only\/never-live\//);
  const config = JSON.parse(result.configuration.text), pair = JSON.parse(result.pair.text);
  assert.deepEqual(config.release_owner, f.releaseOwner);
  assert.equal(config.initial_catalog, f.inputs.catalog.toString());
  assert.equal(config.initial_peer_feed, f.inputs.peerFeed.toString());
  assert.equal(pair.catalog, config.initial_catalog);
  assert.equal(pair.peer_feed, config.initial_peer_feed);
  assert.equal(pair.config_sha256, sha(result.configuration.text));
  assert.equal(result.configuration.bytes, Buffer.byteLength(result.configuration.text));
  assert.equal(result.pair.sha256, sha(result.pair.text));
  assert.deepEqual([config.ttl_seconds, config.refresh_before_seconds], [86400, 21600]);
  assert.equal(result.sourceReferences.length, 9);
  const entry = JSON.parse(f.inputs.catalog).entries[0];
  for (const [i, member] of entry.composition.members.entries()) {
    assert.deepEqual(result.sourceReferences[i], {
      path: `tinyrustlm/direct-composition/v1/compositions/${result.compositionId}/members/${member.name}`,
      bytes: member.bytes, sha256: member.sha256.slice(7) });
  }
  for (const [i, name, route] of [[6, "admission", entry.browser_admission.route],
    [7, "qualificationReceipt", entry.qualification.receipt_route], [8, "license", entry.license.route]]) {
    assert.deepEqual(result.sourceReferences[i], { path: route.slice(1), bytes: f.inputs[name].length, sha256: sha(f.inputs[name]) });
  }
  assert.equal(result.metadata.license.sha256, sha(f.inputs.license));
  assert.deepEqual(config.source_references, result.sourceReferences);
});

test("admitted buffers and release identity are snapshotted before async validation", async () => {
  const f = fixture(), expectedLicense = sha(f.inputs.license), expectedOwner = { ...f.releaseOwner };
  const pending = buildHostRenewalInput(f.inputs, f.releaseOwner);
  Object.values(f.inputs).forEach((value) => value.fill(0));
  f.releaseOwner.source_commit = "e".repeat(40);
  const result = await pending;
  assert.equal(result.metadata.license.sha256, expectedLicense);
  assert.deepEqual(JSON.parse(result.configuration.text).release_owner, expectedOwner);
});

test("failed actual qualification remains rejected even when its catalog digest is repaired", async () => {
  const f = fixture();
  change(f, "qualificationReceipt", (value) => { value.qualityStatus = "failed"; });
  change(f, "catalog", (value) => {
    value.entries[0].qualification.receipt_sha256 = `sha256:${sha(f.inputs.qualificationReceipt)}`;
    value.entries[0].qualification.receipt_bytes = f.inputs.qualificationReceipt.length;
  });
  await assert.rejects(buildHostRenewalInput(f.inputs, f.releaseOwner), (error) => error.code === "catalog-qualification-binding");
});

test("renewal rejects signed catalog/feed time mismatch; canonical browser admission rejects fractional timestamps", async () => {
  for (const [i, edit] of [
    (f) => { change(f, "catalog", (value) => { value.generated_at_utc = new Date(Date.parse(value.generated_at_utc) - 1000).toISOString().replace(".000Z", "Z"); }); },
    (f) => { change(f, "catalog", (value) => { value.valid_until_utc = new Date(Date.parse(value.valid_until_utc) + 1000).toISOString().replace(".000Z", "Z"); }); },
    (f) => { change(f, "catalog", (value) => { value.generated_at_utc = value.generated_at_utc.replace("Z", ".000Z"); }); },
    (f) => { change(f, "catalog", (value) => { value.peer_verification_keys[0].not_after_utc = value.peer_verification_keys[0].not_after_utc.replace("Z", ".000Z"); }); }
  ].entries()) {
    const f = fixture(); edit(f); sign(f);
    await assert.rejects(buildHostRenewalInput(f.inputs, f.releaseOwner), (error) => i < 2
      ? error.message === "host-renewal-pair-time-binding" : error.code === "string");
  }
});

test("license routes cannot collide with evidence, discovery, hidden or executable control paths", async () => {
  for (const route of ["/catalog/compositions.v1.json", "/catalog/peer-announcements.v1.json", "/catalog/.htaccess",
    "/catalog/key.php", "/catalog/key.phtml", "/tinyrustlm/direct-composition/anything"]) {
    const f = fixture(); change(f, "catalog", (value) => { value.entries[0].license.route = route; });
    await assert.rejects(buildHostRenewalInput(f.inputs, f.releaseOwner), /host-renewal-evidence-route/);
  }
  const f = fixture(); change(f, "catalog", (value) => { value.entries[0].license.route = value.entries[0].browser_admission.route; });
  await assert.rejects(buildHostRenewalInput(f.inputs, f.releaseOwner), /host-renewal-reference-binding/);
  const newline = fixture(); change(newline, "catalog", (value) => { value.entries[0].license.route = "/licenses/test.txt\n"; });
  await assert.rejects(buildHostRenewalInput(newline.inputs, newline.releaseOwner), (error) => error.code === "string");
});

test("bounded data properties and release owner reject empty, extra, accessor and malformed fields", async () => {
  const f = fixture();
  for (const inputs of [{ ...f.inputs, extra: Buffer.alloc(1) }, { ...f.inputs, license: Buffer.alloc(0) },
    { ...f.inputs, license: Buffer.alloc(1048577) }, { ...f.inputs, license: "not bytes" }]) {
    await assert.rejects(buildHostRenewalInput(inputs, f.releaseOwner), /host-renewal-input-/);
  }
  const accessor = { ...f.inputs };
  Object.defineProperty(accessor, "license", { enumerable: true, get() { throw new Error("getter must not run"); } });
  await assert.rejects(buildHostRenewalInput(accessor, f.releaseOwner), /host-renewal-input-size/);
  for (const releaseOwner of [{ ...owner(), extra: "x" }, { ...owner(), site: "other.example" },
    { ...owner(), source_commit: "HEAD" }, { ...owner(), publisher_commit: "../bad" }, { ...owner(), authority_sha256: "x" },
    { ...owner(), release_id: "bad\nrelease" }, { ...owner(), release_id: "release\n" },
    { ...owner(), source_commit: `${"a".repeat(40)}\n` }, { ...owner(), authority_sha256: `${"d".repeat(64)}\n` }]) {
    await assert.rejects(buildHostRenewalInput(f.inputs, releaseOwner), /host-renewal-owner-/);
  }
});

test("canonical renewal envelope preserves owned bytes and rejects additional key or file input", () => {
  const f = fixture(); assert.deepEqual(parseEnvelope(encode(f)), { inputs: f.inputs, releaseOwner: f.releaseOwner });
  for (const bytes of [Buffer.alloc(0), Buffer.alloc(6 * 1048576 + 1), Buffer.from("{"), Buffer.from([255]), canon(null), canon([]),
    Buffer.from(encode(f).toString().trim()), canon({ ...JSON.parse(encode(f)), signingKey: "forbidden" })]) {
    assert.throws(() => parseEnvelope(bytes), /host-renewal-/);
  }
  for (const encoded of ["", "AA", "AA== ", null, "A".repeat(1398105)]) {
    const value = JSON.parse(encode(f)); value.inputs.license = encoded;
    assert.throws(() => parseEnvelope(canon(value)), /host-renewal-license-/);
  }
});

test("actual renewal CLI accepts bounded metadata stdin and rejects path/secret arguments", () => {
  const tool = path.resolve(__dirname, "../tools/build-host-renewal-input.js");
  const options = { cwd: path.resolve(__dirname, ".."), windowsHide: true, encoding: "utf8", timeout: 10000,
    maxBuffer: 1024 * 1024, env: { ...process.env, NODE_V8_COVERAGE: "" } };
  const result = spawnSync(process.execPath, [tool], { ...options, input: encode(fixture()) });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(result.stdout).publicationAllowed, false);
  for (const [args, input] of [[["--key", "forbidden"], Buffer.alloc(0)], [[], Buffer.alloc(6 * 1048576 + 1)]]) {
    const failed = spawnSync(process.execPath, [tool, ...args], { ...options, input });
    assert.equal(failed.status, 1); assert.equal(failed.stdout, ""); assert.match(failed.stderr, /host-renewal-/);
  }
});

test("actual pinned PHP core accepts producer bytes, exact references and Ed25519 initial pair", {
  skip: process.env.MINIMODEL_HOST_RENEWAL_PHP_DOCKER !== "1" ? "Explicit cached PHP runtime not selected" : false
}, async () => {
  const f = fixture(), prepared = await buildHostRenewalInput(f.inputs, f.releaseOwner);
  const php = 'require "/source/deploy/host-renewal/renewal-core.php";'
    + '$in=json_decode(stream_get_contents(STDIN));$config=mmRenewParse($in->config);$pair=mmRenewParse($in->pair);'
    + '$c=mmRenewParse($config->initial_catalog);$f=mmRenewParse($config->initial_peer_feed);'
    + '$caps=mmRenewReferences($c->entries[0],$config->source_references);'
    + 'mmRenewPair($pair,$config,hash("sha256",$in->config),$c,$f,time());'
    + 'echo json_encode(["references"=>count($caps),"modelLimit"=>$caps[0],"pairVerified"=>true]),"\\n";';
  const image = "sha256:f62df9916bc05ae3c5120b41a115938f22623b0c98c815cbe2c67bc73cec21ef";
  const result = spawnSync("docker", ["run", "--rm", "-i", "--pull=never", "--network", "none", "--read-only",
    "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--mount",
    `type=bind,source=${path.resolve(__dirname, "..")},target=/source,readonly`, image, "php", "-r", php], {
    input: canon({ config: prepared.configuration.text, pair: prepared.pair.text }), encoding: "utf8",
    windowsHide: true, timeout: 30000, maxBuffer: 4096 });
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.equal(result.stderr, "");
  assert.deepEqual(JSON.parse(result.stdout), { references: 9, modelLimit: 2147483648, pairVerified: true });
});
