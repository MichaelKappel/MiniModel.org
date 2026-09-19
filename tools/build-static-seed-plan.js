"use strict";

const crypto = require("node:crypto");
const catalog = require("../catalog.js");

const MAX_DESCRIPTOR_BYTES = 8192;
const PROFILE_ID = "org.minimodel.acg2/2.0";
const PROFILE_SHA256 = "944ed9b5b0ee2605ebb9ac13f2e5388c181bc857fdbfc082e62e9ae7b07c79be";
const MAX_MEMBER_BYTES = 68_719_476_736;
const MAX_COMPOSITION_BYTES = 68_787_701_384;
const COMPOSITION_BYTES = 896;
const APP_ORIGIN = "https://tinyrustlm.com";
const SEED_ORIGIN = "https://minimodel.org";
const EXPOSED_HEADERS = "Content-Length, Content-Type, Content-Encoding, X-TinyRustLM-Composition-Id, X-TinyRustLM-Piece-Set-Id, X-TinyRustLM-Member-Name, X-TinyRustLM-Member-Id";

/** Keep malformed input out of generated paths/directives without echoing its contents. */
function requireValue(condition, code) {
  if (!condition) throw new Error(code);
}

/** Compute the exact bytes' lowercase digest, including the descriptor's final LF. */
function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

/** Admit the existing Rust/companion metadata contract; this cannot inspect model weights. */
function parseDescriptor(bytes) {
  requireValue(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= MAX_DESCRIPTOR_BYTES,
    "static-seed-descriptor-size");
  let value;
  try { value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)); }
  catch { throw new Error("static-seed-descriptor-json"); }
  requireValue(value !== null && typeof value === "object" && !Array.isArray(value),
    "static-seed-descriptor-contract");
  requireValue(value.schema === "tinyrustlm.direct-composition-seed/1" &&
    value.protocol === catalog.DIRECT_COMPOSITION_PROTOCOL_PROFILE &&
    value.compositionProfile === PROFILE_ID && value.compositionProfileSha256 === PROFILE_SHA256 &&
    typeof value.compositionId === "string" && /^sha256:[a-f0-9]{64}$/.test(value.compositionId) &&
    Array.isArray(value.members) && value.members.length === catalog.MEMBER_NAMES.length,
  "static-seed-descriptor-contract");
  let total = 0;
  const members = value.members.map((member, index) => {
    requireValue(member !== null && typeof member === "object" && !Array.isArray(member) &&
      member.name === catalog.MEMBER_NAMES[index] && Number.isSafeInteger(member.bytes) &&
      member.bytes > 0 && member.bytes <= MAX_MEMBER_BYTES &&
      typeof member.sha256 === "string" && /^[a-f0-9]{64}$/.test(member.sha256) &&
      (index !== 5 || member.bytes === COMPOSITION_BYTES), "static-seed-descriptor-member");
    total += member.bytes;
    requireValue(total <= MAX_COMPOSITION_BYTES, "static-seed-descriptor-total");
    return { name: member.name, bytes: member.bytes, sha256: member.sha256 };
  });
  // Reconstruct field order to reject unknown/duplicate fields and alternate encodings.
  const canonical = { schema: value.schema, protocol: value.protocol,
    compositionProfile: value.compositionProfile, compositionProfileSha256: value.compositionProfileSha256,
    compositionId: value.compositionId, members };
  requireValue(Buffer.from(`${JSON.stringify(canonical)}\n`, "utf8").equals(bytes),
    "static-seed-descriptor-canonical");
  return canonical;
}

/** Replace both Apache header tables so inherited headers cannot create duplicate identities. */
function fixedHeader(name, value) {
  return [`Header onsuccess unset ${name}`, `Header always set ${name} "${value}"`];
}

/** Compile bounded descriptor bytes to reviewable static-host configuration, with no I/O. */
function buildStaticSeedPlan(descriptorBytes) {
  const descriptor = parseDescriptor(descriptorBytes);
  const descriptorSha256 = sha256(descriptorBytes);
  const pieceSetId = `sha256:${descriptorSha256}`;
  // Remote URL directory only: the colon is never materialized as a local Windows path.
  const remoteDirectory = `tinyrustlm/direct-composition/v1/compositions/${descriptor.compositionId}/members`;
  const lines = [
    "# Generated preparation only; verify host behavior and qualified bytes before publication.",
    "Options -Indexes -MultiViews",
    "DirectoryIndex disabled",
    "RewriteEngine On",
    "RewriteCond %{QUERY_STRING} !^$",
    "RewriteRule ^ - [R=404,L]",
    "RewriteCond %{REQUEST_METHOD} !^(GET|HEAD)$",
    "RewriteRule ^ - [R=405,L]",
    `RewriteRule !^(${catalog.MEMBER_NAMES.map((name) => name.replaceAll(".", "\\.")).join("|")})$ - [R=404,L]`,
    "SetHandler default-handler",
    "ForceType application/octet-stream",
    "SetEnv no-gzip 1",
    "SetEnv no-brotli 1",
    "# The server derives Content-Length from real bytes; never forge it from metadata.",
    "# Do not strip Content-Encoding: readback must reject any encoded response.",
    "Header onsuccess unset Access-Control-Allow-Credentials",
    "Header always unset Access-Control-Allow-Credentials",
    ...fixedHeader("Access-Control-Allow-Origin", APP_ORIGIN),
    ...fixedHeader("Access-Control-Allow-Methods", "GET, HEAD"),
    ...fixedHeader("Allow", "GET, HEAD"),
    ...fixedHeader("Access-Control-Expose-Headers", EXPOSED_HEADERS),
    // Browser composition storage owns persistence/retirement, avoiding a second HTTP cache copy.
    ...fixedHeader("Cache-Control", "no-store, no-transform"),
    ...fixedHeader("X-Content-Type-Options", "nosniff"),
    ...fixedHeader("X-TinyRustLM-Composition-Id", descriptor.compositionId),
    ...fixedHeader("X-TinyRustLM-Piece-Set-Id", pieceSetId),
  ];
  const files = descriptor.members.map((member) => {
    lines.push(`<Files "${member.name}">`,
      ...fixedHeader("X-TinyRustLM-Member-Name", member.name).map((line) => `  ${line}`),
      ...fixedHeader("X-TinyRustLM-Member-Id", `sha256:${member.sha256}`).map((line) => `  ${line}`),
      "</Files>");
    const remotePath = `${remoteDirectory}/${member.name}`;
    return { name: member.name, remotePath, url: `${SEED_ORIGIN}/${remotePath}`,
      bytes: member.bytes, sha256: member.sha256,
      expectedHeaders: { "content-type": "application/octet-stream",
        "content-length": String(member.bytes), "content-encoding": null,
        "access-control-allow-origin": APP_ORIGIN,
        "x-tinyrustlm-composition-id": descriptor.compositionId,
        "x-tinyrustlm-piece-set-id": pieceSetId,
        "x-tinyrustlm-member-name": member.name,
        "x-tinyrustlm-member-id": `sha256:${member.sha256}` } };
  });
  const text = `${lines.join("\n")}\n`;
  const bytes = Buffer.from(text, "utf8");
  return { schema: "minimodel.static-seed-preparation.v1", status: "prepared-not-verified-or-published",
    publicationAllowed: false, origin: SEED_ORIGIN, compositionId: descriptor.compositionId,
    descriptorSha256, pieceSetId, remoteDirectory, files,
    configuration: { remotePath: `${remoteDirectory}/.htaccess`, bytes: bytes.length,
      sha256: sha256(bytes), text } };
}

/** Accept one bounded descriptor on stdin; emit only metadata, with no files or network changes. */
async function main() {
  requireValue(process.argv.length === 2, "usage: node tools/build-static-seed-plan.js < canonical-seed-descriptor.json");
  const chunks = [];
  let length = 0;
  for await (const chunk of process.stdin) {
    length += chunk.length;
    requireValue(length <= MAX_DESCRIPTOR_BYTES, "static-seed-descriptor-size");
    chunks.push(chunk);
  }
  process.stdout.write(`${JSON.stringify(buildStaticSeedPlan(Buffer.concat(chunks, length)))}\n`);
}

module.exports = Object.freeze({ buildStaticSeedPlan });
if (require.main === module) {
  main().catch((error) => { process.stderr.write(`${error.message}\n`); process.exitCode = 1; });
}
