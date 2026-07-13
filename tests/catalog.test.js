"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const catalogApi = require("../catalog.js");

const repositoryRoot = path.resolve(__dirname, "..");
const siteOrigin = "https://minimodel.org";

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(repositoryRoot, relativePath), "utf8"));
}

function currentCatalog() {
  return readJson("catalog/models.v0.json");
}

function announcementFor(entry, peerBaseUrl) {
  return {
    announcement_kind: "minimodel.peer_source_announcement",
    announcement_version: 0,
    created_utc: "2026-07-13T00:00:00Z",
    announcement_status: "peer-source-available-metadata-only",
    model_id: entry.model_id,
    artifact_kind: "slm",
    artifact_byte_count: entry.artifact_byte_count,
    artifact_sha256: entry.artifact_sha256,
    peer_offer_route: entry.peer_offer_route,
    peer_offer_sha256: entry.peer_offer_sha256,
    peer_offer_merkle_root_sha256: entry.peer_offer_merkle_root_sha256,
    peer_offer_chunk_count: entry.peer_offer_chunk_count,
    catalog_entry_sha256: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    project_server_serves_model_bytes: false,
    project_server_runs_model_bytes: false,
    project_server_proxies_model_bytes: false,
    peer_source: {
      source_kind: "known-direct-http-peer-v0",
      transport: "direct-http-piece-server-v0",
      peer_base_url: peerBaseUrl,
      consent_status: "publisher-declared-consenting-peer",
      project_server_serves_model_bytes: false,
      project_server_runs_model_bytes: false
    }
  };
}

function feedFor(announcements) {
  return {
    announcement_catalog_version: 0,
    announcement_catalog_kind: "minimodel.peer_source_announcement_catalog",
    generated_at: "2026-07-13T00:00:00Z",
    publisher_id: "minimodel.org",
    publisher_key_id: "minimodel-development-key-v0",
    project_server_serves_model_bytes: false,
    project_server_runs_model_bytes: false,
    project_server_proxies_model_bytes: false,
    announcements
  };
}

test("current catalog renders every entry with description, source, and honest availability", () => {
  const catalog = currentCatalog();
  const views = catalogApi.buildCatalogView(catalog, [], { siteOrigin });

  assert.equal(views.length, catalog.entries.length);
  assert.equal(views.length, 1);
  assert.equal(views[0].modelId, "tinyrustlm/tiny-test-model-q8");
  assert.match(views[0].description, /not a conversational model/i);
  assert.equal(views[0].sourceModel, "tinyrustlm fixture");
  assert.equal(views[0].activePeerCount, 0);
  assert.equal(views[0].availabilityLabel, "Share-ready, no active peer");
  assert.match(views[0].qualityBoundary, /runtime smoke only/i);
});

test("checksum-bound announcements create active availability and deduplicate peer URLs", () => {
  const catalog = currentCatalog();
  const entry = catalog.entries[0];
  const feed = feedFor([
    announcementFor(entry, "https://peer-one.example/pieces"),
    announcementFor(entry, "https://peer-one.example/pieces/"),
    announcementFor(entry, "https://peer-two.example/pieces")
  ]);
  const announcements = catalogApi.validateAnnouncementFeed(feed, catalog, { siteOrigin });
  const views = catalogApi.buildCatalogView(catalog, announcements, { siteOrigin });

  assert.equal(views[0].availability, "active");
  assert.equal(views[0].activePeerCount, 2);
  assert.equal(views[0].availabilityLabel, "2 active peers");
  assert.equal(Object.prototype.hasOwnProperty.call(views[0], "peerUrls"), false);
});

test("source-backed entries get a useful fallback description", () => {
  const catalog = currentCatalog();
  const entry = catalog.entries[0];
  delete entry.description;
  entry.source_huggingface_model_id = "ExampleOrg/Tiny-Source";

  const [view] = catalogApi.buildCatalogView(catalog, [], { siteOrigin });

  assert.equal(view.sourceKind, "Hugging Face");
  assert.equal(view.sourceModel, "ExampleOrg/Tiny-Source");
  assert.match(view.description, /converted from Hugging Face source ExampleOrg\/Tiny-Source/);
});

test("catalog validation rejects project-hosted model bytes", () => {
  const catalog = currentCatalog();
  catalog.entries[0].project_server_serves_model_bytes = true;

  assert.throws(
    () => catalogApi.validateCatalog(catalog, { siteOrigin }),
    /project_server_serves_model_bytes=false/
  );
});

test("announcement binding drift fails closed before availability is applied", () => {
  const catalog = currentCatalog();
  const entry = catalog.entries[0];
  const announcement = announcementFor(entry, "https://peer.example/pieces");
  announcement.artifact_sha256 = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

  assert.throws(
    () => catalogApi.validateAnnouncementFeed(feedFor([announcement]), catalog, { siteOrigin }),
    /artifact_sha256 does not match/
  );
  const [view] = catalogApi.buildCatalogView(catalog, [], { siteOrigin });
  assert.equal(view.activePeerCount, 0);
});

test("filters search descriptions and separate active models from share-ready models", () => {
  const views = [
    {
      modelId: "org/active",
      displayName: "Active Model",
      description: "Conversational model",
      sourceModel: "org/source-a",
      quantization: "q8_0",
      availability: "active"
    },
    {
      modelId: "org/ready",
      displayName: "Ready Model",
      description: "Code completion model",
      sourceModel: "org/source-b",
      quantization: "q4_0",
      availability: "share-ready"
    }
  ];

  assert.deepEqual(catalogApi.filterCatalogViews(views, "code", "all").map((view) => view.modelId), ["org/ready"]);
  assert.deepEqual(catalogApi.filterCatalogViews(views, "", "active").map((view) => view.modelId), ["org/active"]);
  assert.deepEqual(catalogApi.filterCatalogViews(views, "source-b", "share-ready").map((view) => view.modelId), ["org/ready"]);
});

test("published announcement feed hash matches the catalog binding", async () => {
  const catalog = currentCatalog();
  const feedText = fs.readFileSync(path.join(repositoryRoot, "catalog/peer-source-announcements.v0.json"), "utf8");
  const actual = await catalogApi.sha256Prefixed(feedText, crypto.webcrypto);

  assert.equal(actual, catalog.peer_source_announcement_routes[0].sha256);
});
