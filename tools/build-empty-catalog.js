"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { parseArgs } = require("node:util");
const api = require("../catalog.js");

const catalogPath = path.resolve(__dirname, "../catalog/compositions.v1.json");
const peerFeedPath = path.resolve(__dirname, "../catalog/peer-announcements.v1.json");
const canonical = (value) => `${JSON.stringify(value)}\n`;

function buildEmptyCatalog({ revision, generatedAtUtc, validUntilUtc }, options = {}) {
  const catalog = {
    schema: api.CATALOG_SCHEMA,
    catalog_id: "minimodel.org/prelaunch",
    revision,
    generated_at_utc: generatedAtUtc,
    valid_until_utc: validUntilUtc,
    project_server_serves_model_bytes: false,
    project_server_proxies_model_bytes: false,
    project_server_runs_inference: false,
    qualification_policy: "exact-current-evidence-required",
    qualification_receipt_schema: api.QUALIFICATION_RECEIPT_SCHEMA,
    browser_admission_schema: api.BROWSER_ADMISSION_SCHEMA,
    peer_verification_keys: [],
    revoked_qualification_receipt_ids: [],
    supersessions: [],
    entries: [],
  };
  const peerFeed = {
    schema: api.PEER_SCHEMA,
    feed_id: "minimodel.org/prelaunch-peers",
    revision,
    generated_at_utc: generatedAtUtc,
    valid_until_utc: validUntilUtc,
    project_server_serves_model_bytes: false,
    project_server_proxies_model_bytes: false,
    announcements: [],
  };
  api.validatePeerFeed(peerFeed, catalog, options);
  return { catalog, peerFeed };
}

function prepareEmptyCatalog(previousCatalog, previousPeerFeed, parameters, options = {}) {
  // Expired source can be a predecessor, never current authority or renewed evidence.
  api.validatePeerFeed(previousPeerFeed, previousCatalog, {
    now: new Date(previousPeerFeed.generated_at_utc),
  });
  if (previousCatalog.catalog_id !== "minimodel.org/prelaunch"
      || previousPeerFeed.feed_id !== "minimodel.org/prelaunch-peers"
      || [previousCatalog.entries, previousCatalog.peer_verification_keys,
        previousCatalog.revoked_qualification_receipt_ids, previousCatalog.supersessions,
        previousPeerFeed.announcements].some((items) => items.length !== 0)) {
    throw new Error("empty generation cannot discard entries, keys, revocations, supersessions, or announcements");
  }
  const next = buildEmptyCatalog(parameters, options);
  for (const previous of [previousCatalog, previousPeerFeed]) {
    if (parameters.revision <= previous.revision
        || Date.parse(parameters.generatedAtUtc) <= Date.parse(previous.generated_at_utc)) {
      throw new Error("generation requires a strictly newer revision and generation time for both documents");
    }
  }
  return next;
}

function main() {
  const { values } = parseArgs({ options: {
    check: { type: "boolean" },
    revision: { type: "string" },
    "generated-at-utc": { type: "string" },
    "valid-until-utc": { type: "string" },
    document: { type: "string" },
  } });
  const catalogText = fs.readFileSync(catalogPath, "utf8");
  const peerFeedText = fs.readFileSync(peerFeedPath, "utf8");
  const catalog = api.parseCanonicalJsonText(catalogText);
  const peerFeed = api.parseCanonicalJsonText(peerFeedText);
  if (values.check) {
    if (Object.keys(values).length !== 1) throw new Error("--check cannot be combined with generation options");
    const expected = buildEmptyCatalog({
      revision: catalog.revision,
      generatedAtUtc: catalog.generated_at_utc,
      validUntilUtc: catalog.valid_until_utc,
    });
    if (catalogText !== canonical(expected.catalog) || peerFeedText !== canonical(expected.peerFeed)) {
      throw new Error("checked-in metadata differs from the empty generator");
    }
    process.stdout.write("Verified fresh canonical empty catalog and peer feed (local only).\n");
    return;
  }
  if (!/^[1-9][0-9]*$/u.test(values.revision || "")
      || !["catalog", "peerFeed"].includes(values.document)) {
    throw new Error("provide --revision, --generated-at-utc, --valid-until-utc, and --document catalog|peerFeed");
  }
  const generated = prepareEmptyCatalog(catalog, peerFeed, {
    revision: Number(values.revision),
    generatedAtUtc: values["generated-at-utc"],
    validUntilUtc: values["valid-until-utc"],
  });
  // Stdout only: no automatic renewal, source writes, signing, credentials, or publication.
  process.stdout.write(canonical(generated[values.document]));
}

module.exports = { buildEmptyCatalog, prepareEmptyCatalog };
if (require.main === module) {
  try { main(); } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}
