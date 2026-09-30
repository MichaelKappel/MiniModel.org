"use strict";

const crypto = require("node:crypto");
const { admitStaticSeedPublication, parseEnvelope: parseSeedEnvelope } = require("./admit-static-seed-publication.js");

const INPUT_LIMITS = Object.freeze({ descriptor: 8192, catalog: 1048576, peerFeed: 1048576,
  admission: 65536, qualificationReceipt: 131072, license: 1048576 });
const OWNER_FIELDS = Object.freeze(["site", "release_id", "source_commit", "publisher_commit", "manifest_sha256", "authority_sha256"]);
const MEMBER_LIMITS = Object.freeze([2147483648, 67108864, 1048576, 65800, 512, 896]);
const MAX_ENVELOPE_BYTES = 6 * 1048576;
const canonical = (value) => `${JSON.stringify(value)}\n`;
const sha256 = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");

/** Report only a stable error code, never document or credential contents. */
function requireValue(condition, code) {
  if (!condition) throw new Error(code);
}

/** Own exact data properties before asynchronous admission can yield to a caller. */
function snapshot(input, releaseOwner) {
  requireValue(input && Object.keys(input).join(",") === Object.keys(INPUT_LIMITS).join(","), "host-renewal-input-fields");
  const bytes = Object.fromEntries(Object.entries(INPUT_LIMITS).map(([name, limit]) => {
    const property = Object.getOwnPropertyDescriptor(input, name);
    requireValue(property && Object.hasOwn(property, "value") && Buffer.isBuffer(property.value)
      && property.value.length > 0 && property.value.length <= limit, "host-renewal-input-size");
    return [name, Buffer.from(property.value)];
  }));
  requireValue(releaseOwner && Object.keys(releaseOwner).join(",") === OWNER_FIELDS.join(","), "host-renewal-owner-fields");
  const owner = Object.fromEntries(OWNER_FIELDS.map((name) => {
    const property = Object.getOwnPropertyDescriptor(releaseOwner, name);
    requireValue(property && Object.hasOwn(property, "value") && typeof property.value === "string", "host-renewal-owner-value");
    return [name, property.value];
  }));
  requireValue(owner.site === "minimodel.org" && /^[A-Za-z0-9][A-Za-z0-9.@_-]{0,127}$/.test(owner.release_id)
    && !/\s/.test(owner.release_id)
    && [owner.source_commit, owner.publisher_commit].every((value) => value.length === 40 && /^[a-f0-9]{40}$/.test(value))
    && [owner.manifest_sha256, owner.authority_sha256].every((value) => value.length === 64 && /^[a-f0-9]{64}$/.test(value)), "host-renewal-owner-value");
  return { bytes, owner };
}

/** Keep evidence paths disjoint from executable/control files and the six-member seed namespace. */
function evidencePath(route) {
  requireValue(typeof route === "string" && route.length <= 512 && !/[\s\u0000-\u001f\u007f]/u.test(route)
    && /^\/[A-Za-z0-9_-][A-Za-z0-9._/-]*$/.test(route), "host-renewal-evidence-route");
  const parts = route.slice(1).split("/");
  requireValue(parts.every((part) => part && !part.startsWith("."))
    && !route.startsWith("/tinyrustlm/direct-composition/")
    && !["/catalog/compositions.v1.json", "/catalog/peer-announcements.v1.json"].includes(route)
    && !/\.(?:php[0-9]?|phtml|phar)$/i.test(route), "host-renewal-evidence-route");
  return route.slice(1);
}

/** Match the PHP renewal wire format without changing any admitted timestamp or signature. */
function wholeSecond(value) {
  requireValue(typeof value === "string" && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(value)
    && Number.isFinite(Date.parse(value)) && new Date(value).toISOString().replace(".000Z", "Z") === value,
  "host-renewal-time-format");
}

/** Prepare private configuration from actually admitted public bytes; this cannot authorize publication. */
async function buildHostRenewalInput(input, releaseOwner, now = new Date()) {
  const { bytes, owner } = snapshot(input, releaseOwner);
  const seedInput = Object.fromEntries(Object.keys(INPUT_LIMITS).filter((name) => name !== "license").map((name) => [name, bytes[name]]));
  const admitted = await admitStaticSeedPublication(seedInput, now);
  const catalog = JSON.parse(bytes.catalog), feed = JSON.parse(bytes.peerFeed);
  requireValue(catalog.peer_verification_keys.length === 1, "host-renewal-single-key");
  const key = catalog.peer_verification_keys[0], announcement = feed.announcements[0];
  for (const value of [catalog.generated_at_utc, catalog.valid_until_utc, feed.generated_at_utc,
    feed.valid_until_utc, key.not_before_utc, key.not_after_utc, announcement.issued_at_utc, announcement.expires_utc]) wholeSecond(value);
  requireValue(catalog.generated_at_utc === feed.generated_at_utc && catalog.valid_until_utc === feed.valid_until_utc
    && announcement.issued_at_utc === catalog.generated_at_utc && announcement.expires_utc === catalog.valid_until_utc,
  "host-renewal-pair-time-binding");
  const entry = catalog.entries[0];
  const refs = admitted.plan.files.map((file, index) => {
    requireValue(file.bytes <= MEMBER_LIMITS[index] && (index !== 5 || file.bytes === 896), "host-renewal-member-size");
    return { path: file.remotePath, bytes: file.bytes, sha256: file.sha256 };
  });
  for (const [name, route] of [["admission", admitted.publicDocumentRoutes.admission],
    ["qualificationReceipt", admitted.publicDocumentRoutes.qualificationReceipt], ["license", entry.license.route]]) {
    refs.push({ path: evidencePath(route), bytes: bytes[name].length, sha256: sha256(bytes[name]) });
  }
  requireValue(entry.license.review_status === "reviewed" && new Set(refs.map((ref) => ref.path)).size === 9, "host-renewal-reference-binding");
  const configurationText = canonical({ schema: "minimodel.metadata_renewal_config.v1",
    initial_catalog: bytes.catalog.toString("utf8"), initial_peer_feed: bytes.peerFeed.toString("utf8"),
    release_owner: owner, source_references: refs, ttl_seconds: 86400, refresh_before_seconds: 21600 });
  const configurationHash = sha256(Buffer.from(configurationText));
  const pairText = canonical({ schema: "minimodel.metadata_pair.v1", config_sha256: configurationHash,
    catalog: bytes.catalog.toString("utf8"), peer_feed: bytes.peerFeed.toString("utf8") });
  const file = (text) => {
    const data = Buffer.from(text);
    requireValue(data.length <= 1048576, "host-renewal-private-document-size");
    return { text, bytes: data.length, sha256: sha256(data) };
  };
  return { schema: "minimodel.host-renewal-input/1", status: "metadata-admitted-host-and-key-unverified", publicationAllowed: false,
    admittedAtUtc: admitted.admittedAtUtc, compositionId: admitted.compositionId,
    qualificationReceiptId: admitted.qualificationReceiptId, qualityClaim: admitted.qualityClaim, qualityScope: admitted.qualityScope,
    configuration: file(configurationText), pair: file(pairText), sourceReferences: refs,
    metadata: { ...admitted.metadata, license: { bytes: bytes.license.length, sha256: sha256(bytes.license) } } };
}

/** Bounded canonical stdin carries public bytes and release identity; no paths or secrets are accepted. */
function parseEnvelope(bytes) {
  requireValue(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= MAX_ENVELOPE_BYTES, "host-renewal-envelope-size");
  let value;
  try { value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)); }
  catch { throw new Error("host-renewal-envelope-json"); }
  requireValue(value && Object.keys(value).join(",") === "inputs,releaseOwner"
    && Buffer.from(canonical(value)).equals(bytes), "host-renewal-envelope-fields");
  requireValue(value.inputs && Object.keys(value.inputs).join(",") === Object.keys(INPUT_LIMITS).join(","), "host-renewal-envelope-fields");
  const seedEnvelope = {};
  for (const name of Object.keys(INPUT_LIMITS).filter((name) => name !== "license")) seedEnvelope[name] = value.inputs[name];
  const inputs = parseSeedEnvelope(Buffer.from(canonical(seedEnvelope)));
  const encoded = value.inputs.license;
  requireValue(typeof encoded === "string" && encoded.length <= 4 * Math.ceil(INPUT_LIMITS.license / 3), "host-renewal-license-size");
  const license = Buffer.from(encoded, "base64");
  requireValue(license.length > 0 && license.length <= INPUT_LIMITS.license && license.toString("base64") === encoded, "host-renewal-license-base64");
  return { inputs: { ...inputs, license }, releaseOwner: value.releaseOwner };
}

async function main() {
  requireValue(process.argv.length === 2, "host-renewal-arguments");
  let size = 0;
  const chunks = [];
  for await (const chunk of process.stdin) {
    size += chunk.length;
    requireValue(size <= MAX_ENVELOPE_BYTES, "host-renewal-envelope-size");
    chunks.push(chunk);
  }
  const { inputs, releaseOwner } = parseEnvelope(Buffer.concat(chunks, size));
  process.stdout.write(canonical(await buildHostRenewalInput(inputs, releaseOwner)));
}

module.exports = Object.freeze({ buildHostRenewalInput, parseEnvelope });
if (require.main === module) main().catch((error) => {
  process.stderr.write(`${error.code || error.message}\n`); process.exitCode = 1;
});
