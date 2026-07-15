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

function announcementFor(entry, peerBaseUrl, overrides = {}) {
  const normalizedPeerUrl = peerBaseUrl.endsWith("/") ? peerBaseUrl : `${peerBaseUrl}/`;
  const checkedUtc = overrides.checkedUtc || "2026-07-13T00:00:00Z";
  const expiresUtc = overrides.expiresUtc || "2026-07-13T00:30:00Z";
  const laneId = overrides.laneId || "seed-a";
  return {
    announcement_kind: "minimodel.peer_source_announcement",
    announcement_version: 1,
    created_utc: checkedUtc,
    announcement_status: "peer-source-externally-verified-metadata-only",
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
    },
    external_reachability: {
      proof_kind: "minimodel.external_reachability_proof",
      proof_version: 0,
      receipt_id: `receipt-${laneId}`,
      receipt_sha256: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      observer_id: "outside-observer",
      observer_key_id: "outside-observer-key",
      observer_public_key_sha256: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
      fleet_id: "fixture-seed-fleet",
      lane_id: laneId,
      model_id: entry.model_id,
      public_base_url: normalizedPeerUrl,
      artifact_sha256: entry.artifact_sha256,
      piece_set_sha256: "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
      checked_utc: checkedUtc,
      expires_utc: expiresUtc,
      checked_route_count: 4,
      probe_implementation_version: "fixture-observer-v1",
      network_scope: "outside-host-and-local-network",
      signature_payload_sha256: "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
      externally_reachable: true,
      announcement_eligible: true,
      project_server_serves_model_bytes: false,
      project_server_runs_model_bytes: false,
      project_server_proxies_model_bytes: false
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
  const fixture = views.find((view) => view.modelId === "tinyrustlm/tiny-test-model-q8");
  const qwen = views.find((view) => view.modelId === "tinyrustlm/qwen2.5-1.5b-instruct-q4");

  assert.equal(views.length, catalog.entries.length);
  assert.equal(views.length, 2);
  assert.equal(views[0].modelId, "tinyrustlm/qwen2.5-1.5b-instruct-q4");
  assert.equal(views[1].modelId, "tinyrustlm/tiny-test-model-q8");

  assert.ok(fixture);
  assert.match(fixture.description, /not a conversational model/i);
  assert.equal(fixture.sourceModel, "tinyrustlm fixture");
  assert.equal(fixture.catalogRole, "runtime-fixture");
  assert.equal(fixture.availability, "fixture");
  assert.equal(fixture.activePeerCount, 0);
  assert.match(fixture.availabilityLabel, /runtime fixture/i);
  assert.match(fixture.qualityBoundary, /runtime smoke only/i);

  assert.ok(qwen);
  assert.equal(qwen.catalogRole, "p2p-model");
  assert.equal(qwen.sourceModel, "Qwen/Qwen2.5-1.5B-Instruct");
  assert.equal(qwen.sourceRevision, "989aa7980e4cf806f80c7fef2b1adb7bc71aa306");
  assert.equal(qwen.artifactByteCount, 969648576);
  assert.equal(qwen.qualityTier, "conversation proven full assistant pending");
  assert.equal(qwen.limitations, "strict json");
  assert.equal(qwen.activePeerCount, 0);
  assert.equal(qwen.availability, "share-ready");
  assert.match(qwen.availabilityLabel, /awaiting current outside-network proof/i);
});

test("current Rust proof-gated announcements create active availability and deduplicate peer URLs", () => {
  const catalog = currentCatalog();
  const entry = catalog.entries.find((candidate) => candidate.model_id === "tinyrustlm/qwen2.5-1.5b-instruct-q4");
  assert.ok(entry);
  const feed = feedFor([
    announcementFor(entry, "https://peer-one.example/pieces", { laneId: "seed-a" }),
    announcementFor(entry, "https://peer-one.example/pieces/", { laneId: "seed-a" }),
    announcementFor(entry, "https://peer-two.example/pieces", { laneId: "seed-b" })
  ]);
  const announcements = catalogApi.validateAnnouncementFeed(feed, catalog, {
    siteOrigin,
    now: new Date("2026-07-13T00:10:00Z")
  });
  const views = catalogApi.buildCatalogView(catalog, announcements, { siteOrigin });

  assert.equal(views[0].availability, "active");
  assert.equal(views[0].activePeerCount, 2);
  assert.equal(views[0].provenLaneCount, 2);
  assert.equal(views[0].availabilityLabel, "2 active peers");
  assert.equal(views[0].oldestProofCheckedUtc, "2026-07-13T00:00:00Z");
  assert.equal(views[0].nextProofExpiryUtc, "2026-07-13T00:30:00Z");
  assert.equal(Object.prototype.hasOwnProperty.call(views[0], "peerUrls"), false);
});

test("legacy metadata-only announcements cannot create active availability", () => {
  const catalog = currentCatalog();
  const announcement = announcementFor(catalog.entries[0], "https://peer.example/pieces");
  announcement.announcement_version = 0;
  announcement.announcement_status = "peer-source-available-metadata-only";
  delete announcement.external_reachability;

  assert.throws(
    () => catalogApi.validateAnnouncementFeed(feedFor([announcement]), catalog, {
      siteOrigin,
      now: new Date("2026-07-13T00:10:00Z")
    }),
    /version 1 external proof/i
  );
});

test("expired or overlong external proof fails closed", () => {
  const catalog = currentCatalog();
  const entry = catalog.entries[0];
  const expired = announcementFor(entry, "https://peer.example/pieces", {
    expiresUtc: "2026-07-13T00:09:59Z"
  });
  const overlong = announcementFor(entry, "https://peer.example/pieces", {
    expiresUtc: "2026-07-13T01:00:01Z"
  });
  for (const announcement of [expired, overlong]) {
    assert.throws(
      () => catalogApi.validateAnnouncementFeed(feedFor([announcement]), catalog, {
        siteOrigin,
        now: new Date("2026-07-13T00:10:00Z")
      }),
      /stale, expired, or outside the one-hour validity boundary/i
    );
  }
});

test("external proof identity drift fails closed", () => {
  const catalog = currentCatalog();
  const announcement = announcementFor(catalog.entries[0], "https://peer.example/pieces");
  announcement.external_reachability.public_base_url = "https://different.example/pieces/";

  assert.throws(
    () => catalogApi.validateAnnouncementFeed(feedFor([announcement]), catalog, {
      siteOrigin,
      now: new Date("2026-07-13T00:10:00Z")
    }),
    /public_base_url does not match/i
  );
});

test("source-backed entries get a useful fallback description", () => {
  const catalog = currentCatalog();
  const entry = catalog.entries[0];
  catalog.entries = [entry];
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
    () => catalogApi.validateAnnouncementFeed(feedFor([announcement]), catalog, {
      siteOrigin,
      now: new Date("2026-07-13T00:10:00Z")
    }),
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
    },
    {
      modelId: "org/fixture",
      displayName: "Runtime Fixture",
      description: "Structural smoke fixture",
      sourceModel: "deterministic fixture",
      quantization: "q8_0",
      availability: "fixture"
    }
  ];

  assert.deepEqual(catalogApi.filterCatalogViews(views, "code", "all").map((view) => view.modelId), ["org/ready"]);
  assert.deepEqual(catalogApi.filterCatalogViews(views, "", "active").map((view) => view.modelId), ["org/active"]);
  assert.deepEqual(catalogApi.filterCatalogViews(views, "source-b", "share-ready").map((view) => view.modelId), ["org/ready"]);
  assert.deepEqual(catalogApi.filterCatalogViews(views, "", "fixture").map((view) => view.modelId), ["org/fixture"]);
});

test("published Qwen catalog metadata routes exist without publishing model bytes", () => {
  const catalog = currentCatalog();
  const entry = catalog.entries.find((candidate) => candidate.model_id === "tinyrustlm/qwen2.5-1.5b-instruct-q4");

  assert.ok(entry);
  assert.equal(entry.project_server_serves_model_bytes, false);
  assert.equal(entry.project_server_runs_model_bytes, false);
  assert.equal(entry.artifact_project_server_url, null);
  for (const routeName of ["model_card_route", "manifest_route", "peer_offer_route", "license_route"]) {
    const route = entry[routeName];
    const resolved = new URL(route, "https://minimodel.org/catalog/models.v0.json");
    assert.equal(resolved.origin, siteOrigin);
    assert.equal(fs.existsSync(path.join(repositoryRoot, resolved.pathname)), true, `${routeName} must exist`);
  }
  assert.equal(fs.existsSync(path.join(repositoryRoot, "models", "qwen2.5-1.5b-instruct-q4.slm")), false);
});

test("published announcement feed hash matches the catalog binding", async () => {
  const catalog = currentCatalog();
  const feedText = fs.readFileSync(path.join(repositoryRoot, "catalog/peer-source-announcements.v0.json"), "utf8");
  const actual = await catalogApi.sha256Prefixed(feedText, crypto.webcrypto);

  assert.equal(actual, catalog.peer_source_announcement_routes[0].sha256);
});

test("public site configuration retires hosted demo bytes and denies every slm route", () => {
  assert.equal(fs.existsSync(path.join(repositoryRoot, "catalog/default-demo.v0.json")), false);
  const apacheRules = fs.readFileSync(path.join(repositoryRoot, ".htaccess"), "utf8");
  assert.match(apacheRules, /RewriteRule \(\^\|\/\)\.\*\\\.slm\$ - \[R=404,L,NC\]/);
});
