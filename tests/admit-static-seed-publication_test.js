"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");
const { admitStaticSeedPublication, parseEnvelope } = require("../tools/admit-static-seed-publication.js");
const catalog = require("../../TinyRustLM.com/tinyrustlm/app/minimodel-catalog-v1.js");
const demo = require("../../TinyRustLM.com/tinyrustlm/tools/synthetic-p2p-interview-demo.js");

const canonical = (value) => Buffer.from(`${JSON.stringify(value)}\n`);
const hash = (value) => crypto.createHash("sha256").update(value).digest("hex");
const encode = (input) => canonical(Object.fromEntries(Object.entries(input).map(([name, bytes]) => [name, bytes.toString("base64")])));

/** Reuse one canonical synthetic producer; no copied runtime, real weights, file output or browser launch. */
function fixture() {
  const source = demo.createSyntheticFixture("static-seed-admission", demo.PRESERVED_PRIOR_LENGTHS, 0);
  const documents = demo.createDemoDocuments([source], 48123);
  const catalogDocument = JSON.parse(JSON.stringify(documents.catalog));
  const feed = JSON.parse(JSON.stringify(documents.peerFeed));
  const admission = JSON.parse(source.admissionText);
  const descriptor = canonical({ schema: "tinyrustlm.direct-composition-seed/1",
    protocol: catalog.DIRECT_COMPOSITION_PROTOCOL_PROFILE, compositionProfile: "org.minimodel.acg2/2.0",
    compositionProfileSha256: admission.artifacts[5].formatProfileSha256,
    compositionId: source.compositionId,
    members: source.members.map((member) => ({ name: member.fileName, bytes: member.byteLength, sha256: member.sha256 })) });
  const { publicKey, privateKey } = crypto.generateKeyPairSync("ed25519");
  catalogDocument.peer_verification_keys[0].public_key_base64url = publicKey.export({ format: "jwk" }).x;
  catalogDocument.project_server_serves_model_bytes = true;
  feed.project_server_serves_model_bytes = true;
  feed.announcements = [feed.announcements[0]];
  catalogDocument.entries[0].peer_announcement_ids = [feed.announcements[0].announcement_id];
  const announcement = feed.announcements[0];
  Object.assign(announcement, { peer_base_url: "https://minimodel.org/", piece_set_sha256: `sha256:${hash(descriptor)}`,
    outside_proof_status: "not-required-operator-seed", outside_proof_receipt_sha256: null });
  announcement.signature = crypto.sign(null, catalog.peerAnnouncementSigningBytes(announcement), privateKey).toString("base64url");
  return { descriptor, catalog: canonical(catalogDocument), peerFeed: canonical(feed),
    admission: Buffer.from(source.admissionText), qualificationReceipt: Buffer.from(source.qualificationReceiptText) };
}

/** Change metadata and keep its wire encoding canonical so validation reaches the intended boundary. */
function mutate(input, name, change) {
  const value = JSON.parse(input[name]);
  change(value);
  input[name] = canonical(value);
  return input;
}

test("admission consumes real canonical browser receipt binding and returns metadata-only seed plan", async () => {
  const input = fixture();
  const result = await admitStaticSeedPublication(input);
  assert.equal(result.publicationAllowed, false);
  assert.equal(result.status, "metadata-admitted-payload-and-host-unverified");
  assert.equal(result.plan.files.length, 6);
  assert.equal(result.pieceSetId, `sha256:${hash(input.descriptor)}`);
  assert.equal(result.metadata.qualificationReceipt.sha256, hash(input.qualificationReceipt));
  assert.equal(result.qualityScope, demo.TEST_ONLY_SCOPE);
  assert.ok(result.plan.files.every((file) => file.url.startsWith("https://minimodel.org/")));
});

test("admission rejects unrelated descriptor, member, piece-set and real receipt bytes", async () => {
  for (const change of [
    (input) => mutate(input, "descriptor", (value) => { value.compositionId = `sha256:${"0".repeat(64)}`; }),
    (input) => mutate(input, "descriptor", (value) => { value.members[0].bytes += 1; }),
    (input) => mutate(input, "catalog", (value) => { value.entries[0].composition.members[0].sha256 = `sha256:${"0".repeat(64)}`; }),
    (input) => mutate(input, "qualificationReceipt", (value) => { value.qualityStatus = "failed"; }),
    (input) => mutate(input, "admission", (value) => { value.modelId = "test-only/other"; }),
    (input) => mutate(input, "peerFeed", (value) => { value.announcements[0].piece_set_sha256 = `sha256:${"0".repeat(64)}`; }),
    (input) => mutate(input, "catalog", (value) => { value.revoked_qualification_receipt_ids = [value.entries[0].qualification.receipt_id]; }),
    (input) => mutate(input, "catalog", (value) => { value.project_server_serves_model_bytes = false; }),
    (input) => mutate(input, "peerFeed", (value) => { value.announcements = []; }),
    (input) => mutate(input, "catalog", (value) => { value.entries = []; })
  ]) await assert.rejects(admitStaticSeedPublication(change(fixture())));
});

test("a rehashed qualification-shaped failure is rejected by the real receipt parser", async () => {
  const input = mutate(fixture(), "qualificationReceipt", (value) => { value.qualityStatus = "failed"; });
  mutate(input, "catalog", (value) => {
    value.entries[0].qualification.receipt_sha256 = `sha256:${hash(input.qualificationReceipt)}`;
    value.entries[0].qualification.receipt_bytes = input.qualificationReceipt.length;
  });
  await assert.rejects(admitStaticSeedPublication(input), (error) => error.code === "catalog-qualification-binding");
});

test("time, byte bounds, exact fields and immutable snapshots are enforced before async work", async () => {
  const input = fixture();
  await assert.rejects(admitStaticSeedPublication(input, new Date("invalid")), /time/);
  await assert.rejects(admitStaticSeedPublication(input, "2026-09-29"), /time/);
  await assert.rejects(admitStaticSeedPublication({ ...input, extra: Buffer.alloc(1) }), /fields/);
  for (const name of Object.keys(input)) {
    await assert.rejects(admitStaticSeedPublication({ ...input, [name]: Buffer.alloc(0) }), /size/);
    await assert.rejects(admitStaticSeedPublication({ ...input, [name]: "not bytes" }), /size/);
  }
  await assert.rejects(admitStaticSeedPublication({ ...input, descriptor: Buffer.alloc(8193) }), /size/);
  const pending = admitStaticSeedPublication(input);
  input.qualificationReceipt.fill(0);
  assert.equal((await pending).publicationAllowed, false);
  await assert.rejects(admitStaticSeedPublication(fixture(), new Date(Date.now() + 86400000)), /stale/);
});

test("canonical envelope preserves exact bytes and rejects aliases, unknown fields and overflow", () => {
  const input = fixture();
  assert.deepEqual(parseEnvelope(encode(input)), input);
  for (const bytes of [Buffer.alloc(0), Buffer.alloc(3 * 1024 * 1024 + 1), Buffer.from("{"),
    Buffer.from("[]\n"), Buffer.from("null\n"), Buffer.from("{}\n"), Buffer.from([255]),
    Buffer.from(encode(input).toString().trim()), canonical({ ...JSON.parse(encode(input)), extra: "AA==" })]) {
    assert.throws(() => parseEnvelope(bytes));
  }
  for (const replacement of ["", "AA", "AA== ", null, "A".repeat(10925)]) {
    const encoded = JSON.parse(encode(input));
    encoded.descriptor = replacement;
    assert.throws(() => parseEnvelope(canonical(encoded)));
  }
});

test("actual metadata-only CLI has bounded stdin and refuses path arguments", () => {
  const tool = path.resolve(__dirname, "../tools/admit-static-seed-publication.js");
  const options = { cwd: path.resolve(__dirname, ".."), windowsHide: true, encoding: "utf8",
    timeout: 10000, maxBuffer: 128 * 1024, env: { ...process.env, NODE_V8_COVERAGE: "" } };
  const result = spawnSync(process.execPath, [tool], { ...options, input: encode(fixture()) });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(result.stdout).publicationAllowed, false);
  for (const [args, input] of [[["--source-root", "outside"], Buffer.alloc(0)], [[], Buffer.alloc(3 * 1024 * 1024 + 1)]]) {
    const failed = spawnSync(process.execPath, [tool, ...args], { ...options, input });
    assert.equal(failed.status, 1);
    assert.equal(failed.stdout, "");
    assert.match(failed.stderr, /static-seed-publication-/);
  }
});
