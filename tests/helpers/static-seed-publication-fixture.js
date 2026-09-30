"use strict";
const crypto = require("node:crypto");
const catalog = require("../../../TinyRustLM.com/tinyrustlm/app/minimodel-catalog-v1.js");
const demo = require("../../../TinyRustLM.com/tinyrustlm/tools/synthetic-p2p-interview-demo.js");
const canonical = (value) => Buffer.from(`${JSON.stringify(value)}\n`);
const hash = (value) => crypto.createHash("sha256").update(value).digest("hex");

/** Reuse one canonical synthetic producer; no copied runtime, real weights, file output or browser launch. */
function createAdmittedSeedFixture({ renewal = false } = {}) {
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
  if (renewal) {
    const utc = (milliseconds) => new Date(Math.floor(milliseconds / 1000) * 1000).toISOString().replace('.000Z', 'Z');
    const now = Date.now();
    catalogDocument.generated_at_utc = feed.generated_at_utc = announcement.issued_at_utc = utc(now - 60000);
    catalogDocument.valid_until_utc = feed.valid_until_utc = announcement.expires_utc = utc(now + 23 * 3600000);
    catalogDocument.peer_verification_keys[0].not_before_utc = utc(now - 86400000);
    catalogDocument.peer_verification_keys[0].not_after_utc = utc(now + 30 * 86400000);
  }
  announcement.signature = crypto.sign(null, catalog.peerAnnouncementSigningBytes(announcement), privateKey).toString("base64url");
  return { privateKey, inputs: { descriptor, catalog: canonical(catalogDocument), peerFeed: canonical(feed),
    admission: Buffer.from(source.admissionText), qualificationReceipt: Buffer.from(source.qualificationReceiptText) } };
}

module.exports = Object.freeze({ createAdmittedSeedFixture });
