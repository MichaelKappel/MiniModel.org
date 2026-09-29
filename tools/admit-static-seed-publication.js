"use strict";

const crypto = require("node:crypto");
const { buildStaticSeedPlan } = require("./build-static-seed-plan.js");
// Runtime/admission semantics have one canonical owner; never copy these parsers.
const catalog = require("../../TinyRustLM.com/tinyrustlm/app/minimodel-catalog-v1.js");
const admissionParser = require("../../TinyRustLM.com/tinyrustlm/app/browser-model-admission.js");
const admissionLoader = require("../../TinyRustLM.com/tinyrustlm/app/catalog-admission-loader.js");

const ORIGIN = "https://minimodel.org";
const MAX_ENVELOPE_BYTES = 3 * 1024 * 1024;
const INPUT_LIMITS = Object.freeze({ descriptor: 8192, catalog: 1024 * 1024, peerFeed: 1024 * 1024,
  admission: 65536, qualificationReceipt: 131072 });

/** Reject malformed metadata without echoing document content into deployment errors. */
function requireValue(condition, code) {
  if (!condition) throw new Error(code);
}

/** Compute the exact byte identity retained by the publication plan. */
function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

/** Own all bounded input bytes before asynchronous signature or admission work. */
function snapshotInputs(input) {
  requireValue(input && Object.keys(input).sort().join(",") === Object.keys(INPUT_LIMITS).sort().join(","),
    "static-seed-publication-fields");
  return Object.fromEntries(Object.entries(INPUT_LIMITS).map(([name, limit]) => {
    const field = Object.getOwnPropertyDescriptor(input, name);
    requireValue(field && Object.hasOwn(field, "value") && Buffer.isBuffer(field.value)
      && field.value.length > 0 && field.value.length <= limit, "static-seed-publication-size");
    return [name, Buffer.from(field.value)];
  }));
}

/** Decode canonical current metadata; invalid UTF-8 cannot acquire a repaired identity. */
function parseDocument(bytes) {
  return catalog.parseCanonicalJsonText(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
}

/** Bind one exact seed to the actual browser qualification/admission reader without any network fetch. */
async function admitStaticSeedPublication(input, now = new Date()) {
  requireValue(now instanceof Date && Number.isFinite(now.getTime()), "static-seed-publication-time");
  const admittedAt = new Date(now.getTime());
  const bytes = snapshotInputs(input);
  const plan = buildStaticSeedPlan(bytes.descriptor);
  const source = parseDocument(bytes.catalog);
  const feed = parseDocument(bytes.peerFeed);
  const views = await catalog.buildVerifiedCatalogView(source, feed, { now: admittedAt, crypto: crypto.webcrypto });
  requireValue(source.project_server_serves_model_bytes === true && feed.project_server_serves_model_bytes === true
    && views.length === 1 && source.entries.length === 1 && feed.announcements.length === 1,
  "static-seed-publication-single-operator");
  const view = views[0];
  requireValue(view.composition.compositionId === plan.compositionId && view.peers.length === 1,
    "static-seed-publication-composition");
  const peer = view.peers[0];
  requireValue(peer.signatureVerified === true && peer.peerBaseUrl === `${ORIGIN}/`
    && peer.outsideProofStatus === "not-required-operator-seed" && peer.outsideProofReceiptSha256 === null
    && peer.pieceSetSha256 === plan.pieceSetId, "static-seed-publication-peer");
  const memberDescriptors = source.entries[0].composition.members;
  requireValue(plan.files.every((file, index) => file.name === memberDescriptors[index].name
    && file.bytes === memberDescriptors[index].bytes && `sha256:${file.sha256}` === memberDescriptors[index].sha256),
  "static-seed-publication-member");
  const documents = new Map([
    [new URL(view.browserAdmission.route, ORIGIN).href, bytes.admission],
    [new URL(view.qualification.receiptRoute, ORIGIN).href, bytes.qualificationReceipt]
  ]);
  requireValue(documents.size === 2, "static-seed-publication-document-routes");
  const loaded = await admissionLoader.load({ catalogView: view, catalogBaseUrl: ORIGIN,
    admissionParser, crypto: crypto.webcrypto,
    // This closed in-memory map is the only fetch implementation. Unknown routes fail.
    fetch: async (url) => {
      const body = documents.get(String(url));
      requireValue(body !== undefined, "static-seed-publication-document-route");
      return new Response(body, { status: 200, headers: { "Content-Type": "application/json",
        "Content-Length": String(body.length) } });
    }
  });
  return Object.freeze({ schema: "minimodel.qualified-static-seed-plan/1",
    status: "metadata-admitted-payload-and-host-unverified", publicationAllowed: false,
    admittedAtUtc: admittedAt.toISOString(), entryId: view.entryId,
    compositionId: plan.compositionId, pieceSetId: plan.pieceSetId, plan,
    qualificationReceiptId: loaded.qualificationReceipt.receiptId,
    qualityClaim: loaded.qualificationReceipt.qualityClaim, qualityScope: loaded.qualificationReceipt.qualityScope,
    metadata: Object.fromEntries(Object.entries(bytes).map(([name, value]) => [name,
      { bytes: value.length, sha256: sha256(value) }])),
    publicDocumentRoutes: { admission: view.browserAdmission.route, qualificationReceipt: view.qualification.receiptRoute },
    catalogAuthority: { id: source.catalog_id, revision: source.revision,
      generatedAtUtc: source.generated_at_utc, validUntilUtc: source.valid_until_utc },
    peerFeedAuthority: { id: feed.feed_id, revision: feed.revision,
      generatedAtUtc: feed.generated_at_utc, validUntilUtc: feed.valid_until_utc }
  });
}

/** Decode only the exact canonical base64 envelope, keeping document bytes unchanged. */
function parseEnvelope(bytes) {
  requireValue(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= MAX_ENVELOPE_BYTES,
    "static-seed-publication-envelope-size");
  let envelope;
  try { envelope = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)); }
  catch { throw new Error("static-seed-publication-envelope-json"); }
  requireValue(envelope && !Array.isArray(envelope)
    && Object.keys(envelope).join(",") === Object.keys(INPUT_LIMITS).join(","),
  "static-seed-publication-envelope-fields");
  const inputs = Object.fromEntries(Object.entries(INPUT_LIMITS).map(([name, maximum]) => {
    const encoded = envelope[name];
    requireValue(typeof encoded === "string" && encoded.length <= 4 * Math.ceil(maximum / 3),
      "static-seed-publication-envelope-field-size");
    const decoded = Buffer.from(encoded, "base64");
    requireValue(decoded.length > 0 && decoded.length <= maximum && decoded.toString("base64") === encoded,
      "static-seed-publication-envelope-base64");
    return [name, decoded];
  }));
  requireValue(Buffer.from(`${JSON.stringify(envelope)}\n`).equals(bytes), "static-seed-publication-envelope-canonical");
  return inputs;
}

/** The CLI admits metadata only from bounded stdin, with no paths, model reads or network fallback. */
async function main() {
  requireValue(process.argv.length === 2, "static-seed-publication-arguments");
  const chunks = [];
  let count = 0;
  for await (const chunk of process.stdin) {
    count += chunk.length;
    requireValue(count <= MAX_ENVELOPE_BYTES, "static-seed-publication-envelope-size");
    chunks.push(chunk);
  }
  const admitted = await admitStaticSeedPublication(parseEnvelope(Buffer.concat(chunks, count)));
  process.stdout.write(`${JSON.stringify(admitted)}\n`);
}

module.exports = Object.freeze({ admitStaticSeedPublication, parseEnvelope });
if (require.main === module) {
  main().catch((error) => { process.stderr.write(`${error.code || error.message}\n`); process.exitCode = 1; });
}
