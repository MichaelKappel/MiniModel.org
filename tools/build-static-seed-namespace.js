"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { buildStaticSeedPlan } = require("./build-static-seed-plan.js");

const MAX_REGISTRY_BYTES = 16_384;

/** Register paths from one exact descriptor without claiming presence or qualification. */
function buildStaticSeedNamespace(bytes) {
  if (!Buffer.isBuffer(bytes) || bytes.length < 1 || bytes.length > MAX_REGISTRY_BYTES) {
    throw new Error("static-seed-namespace-size");
  }
  let value;
  try { value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)); }
  catch { throw new Error("static-seed-namespace-json"); }
  if (!value || typeof value !== "object" || Array.isArray(value) ||
      value.schema !== "minimodel.static-seed-namespace/1" ||
      (value.descriptorBase64 !== null && typeof value.descriptorBase64 !== "string") ||
      !Buffer.from(`${JSON.stringify({ schema: value.schema, descriptorBase64: value.descriptorBase64 })}\n`).equals(bytes)) {
    throw new Error("static-seed-namespace-canonical");
  }
  let plan = null;
  if (value.descriptorBase64 !== null) {
    const descriptor = Buffer.from(value.descriptorBase64, "base64");
    if (descriptor.length === 0 || descriptor.toString("base64") !== value.descriptorBase64) {
      throw new Error("static-seed-namespace-base64");
    }
    plan = buildStaticSeedPlan(descriptor);
  }
  return {
    schema: "minimodel.static-seed-namespace-resolved/1",
    registrySha256: crypto.createHash("sha256").update(bytes).digest("hex"),
    publicationAllowed: false,
    compositionId: plan ? plan.compositionId : null,
    files: plan ? [...plan.files, plan.configuration].map((file) => ({
      path: file.remotePath, bytes: file.bytes, sha256: file.sha256,
    })) : [],
    plan,
  };
}

/** Read only the canonical registry, with a bounded handle and no local seed payload. */
function main(args) {
  if (args.length !== 0) throw new Error("usage: node tools/build-static-seed-namespace.js");
  const registry = path.resolve(__dirname, "../deploy/static-seed-namespace.v1.json");
  for (let current = registry;; current = path.dirname(current)) {
    if (fs.lstatSync(current).isSymbolicLink()) throw new Error("static-seed-namespace-reparse");
    if (path.dirname(current) === current) break;
  }
  const handle = fs.openSync(registry, "r");
  try {
    const stat = fs.fstatSync(handle);
    if (!stat.isFile() || stat.size < 1 || stat.size > MAX_REGISTRY_BYTES) {
      throw new Error("static-seed-namespace-size");
    }
    const bytes = Buffer.alloc(MAX_REGISTRY_BYTES + 1);
    let length = 0;
    while (length < bytes.length) {
      const count = fs.readSync(handle, bytes, length, bytes.length - length, null);
      if (count === 0) break;
      length += count;
    }
    process.stdout.write(`${JSON.stringify(buildStaticSeedNamespace(bytes.subarray(0, length)))}\n`);
  } finally { fs.closeSync(handle); }
}

module.exports = Object.freeze({ buildStaticSeedNamespace });
if (require.main === module) {
  try { main(process.argv.slice(2)); }
  catch (error) { process.stderr.write(`${error.message}\n`); process.exitCode = 1; }
}
