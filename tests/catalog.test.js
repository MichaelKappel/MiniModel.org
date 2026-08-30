"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const repositoryRoot = path.resolve(__dirname, "..");
const catalogApi = require("../catalog.js");

function read(relativePath) {
  return fs.readFileSync(path.join(repositoryRoot, relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

function sha(character) {
  return `sha256:${character.repeat(64)}`;
}

function validEntry() {
  return {
    entry_id: "qwen/qwen3-0.6b",
    display_name: "Qwen3 0.6B",
    description: "One exact qualified six-member SLM2 composition.",
    source: {
      repository: "Qwen/Qwen3-0.6B",
      revision: "a".repeat(40),
      source_lock_sha256: sha("b"),
    },
    license: {
      route: "/licenses/qwen3.txt",
      review_status: "reviewed",
    },
    composition: {
      composition_id: sha("c"),
      member_count: 6,
      members: catalogApi.MEMBER_NAMES.map((name, index) => ({
        name,
        bytes: index + 1,
        sha256: sha(String(index + 1)),
        schema: `tinyrustlm.member.${index + 1}.v1`,
      })),
    },
    profiles: {
      conversion: "tinyrustlm.converter.v1",
      quantization: "tinyrustlm.mixed-q8.v1",
      runtime: "tinyrustlm.browser.v1",
    },
    qualification: {
      status: "qualified",
      receipt_id: "qualification/qwen3-0.6b",
      receipt_sha256: sha("d"),
      numerical_status: "passed",
      browser_status: "passed",
      quality_status: "qualified",
    },
    peer_announcement_ids: [],
  };
}

function validCatalog(entries = []) {
  return {
    schema: catalogApi.CATALOG_SCHEMA,
    catalog_id: "minimodel.org/prelaunch",
    generated_at_utc: "2026-08-28T00:00:00Z",
    project_server_serves_model_bytes: false,
    project_server_proxies_model_bytes: false,
    project_server_runs_inference: false,
    qualification_policy: "exact-current-evidence-required",
    entries,
  };
}

function validPeerFeed(announcements = []) {
  return {
    schema: catalogApi.PEER_SCHEMA,
    feed_id: "minimodel.org/prelaunch-peers",
    generated_at_utc: "2026-08-28T00:00:00Z",
    project_server_serves_model_bytes: false,
    project_server_proxies_model_bytes: false,
    announcements,
  };
}

function validAnnouncement() {
  return {
    announcement_id: "peer/qwen3/lane-1",
    composition_id: sha("c"),
    piece_set_sha256: sha("e"),
    peer_base_url: "https://peer.example.net/models/qwen3/",
    protocol_profile: "minimodel.direct-piece.v1",
    consent_status: "explicit",
    outside_proof_status: "current-passed",
    expires_utc: "2099-01-01T00:00:00Z",
  };
}

test("publishes a truthful empty current SLM2 catalog and peer feed", () => {
  const catalog = readJson("catalog/compositions.v1.json");
  const peers = readJson("catalog/peer-announcements.v1.json");
  assert.equal(catalogApi.validateCatalog(catalog), catalog);
  assert.deepEqual(catalogApi.validatePeerFeed(peers, catalog), []);
  assert.deepEqual(catalogApi.buildCatalogView(catalog, peers), []);
  assert.equal(catalog.entries.length, 0);
  assert.equal(peers.announcements.length, 0);
});

test("projects a qualified composition to explicit pinned local conversion", () => {
  const [view] = catalogApi.buildCatalogView(validCatalog([validEntry()]), validPeerFeed());
  assert.equal(view.availability, "local-conversion");
  assert.equal(view.availabilityLabel, "Pinned local conversion");
  assert.equal(view.peerCount, 0);
});

test("projects only a current explicitly consenting direct peer", () => {
  const entry = validEntry();
  entry.peer_announcement_ids.push("peer/qwen3/lane-1");
  const [view] = catalogApi.buildCatalogView(
    validCatalog([entry]),
    validPeerFeed([validAnnouncement()]),
    { now: new Date("2026-08-28T00:00:00Z") },
  );
  assert.equal(view.availability, "peer");
  assert.equal(view.peerCount, 1);
});

test("rejects unknown catalog fields and project-server model behavior", () => {
  assert.throws(
    () => catalogApi.validateCatalog({ ...validCatalog(), legacy_alias: true }),
    /unknown or missing/,
  );
  assert.throws(
    () => catalogApi.validateCatalog({ ...validCatalog(), project_server_serves_model_bytes: true }),
    /off project servers/,
  );
});

test("rejects incomplete or drifted six-member composition closure", () => {
  const entry = validEntry();
  entry.composition.members.pop();
  entry.composition.member_count = 5;
  assert.throws(
    () => catalogApi.validateCatalog(validCatalog([entry])),
    /exactly six members/,
  );

  const duplicate = validEntry();
  duplicate.composition.members[5].name = duplicate.composition.members[0].name;
  assert.throws(
    () => catalogApi.validateCatalog(validCatalog([duplicate])),
    /member set is not closed/,
  );
});

test("rejects an unqualified catalog entry", () => {
  const entry = validEntry();
  entry.qualification.browser_status = "untested";
  assert.throws(
    () => catalogApi.validateCatalog(validCatalog([entry])),
    /qualification is incomplete/,
  );
});

test("rejects expired and project-hosted peer announcements", () => {
  const catalog = validCatalog([validEntry()]);
  const expired = validAnnouncement();
  expired.expires_utc = "2026-08-27T00:00:00Z";
  assert.throws(
    () => catalogApi.validatePeerFeed(validPeerFeed([expired]), catalog, {
      now: new Date("2026-08-28T00:00:00Z"),
    }),
    /expired/,
  );

  const hosted = validAnnouncement();
  hosted.peer_base_url = "https://minimodel.org/models/";
  assert.throws(
    () => catalogApi.validatePeerFeed(validPeerFeed([hosted]), catalog, {
      now: new Date("2026-08-28T00:00:00Z"),
    }),
    /non-project peer/,
  );
});

test("publishes only current catalog routes and explicitly retires former routes", () => {
  const html = read("index.html");
  const sitemap = read("sitemap.xml");
  const apache = read(".htaccess");
  for (const current of [
    "/catalog/compositions.v1.json",
    "/catalog/peer-announcements.v1.json",
    "/docs/slm2-composition.md",
    "/docs/acquisition-contract.md",
  ]) {
    assert.match(html + sitemap, new RegExp(current.replaceAll(".", "\\.")));
  }
  assert.doesNotMatch(html + sitemap, /models\.v0|manifest\.v0|minimodel-peer-offer/i);
  assert.match(apache, /R=410/);
  assert.match(apache, /models\\\.v0/);
});

test("keeps the two MemoryEndpoints uses separate from model transport", () => {
  const memory = read("docs/memory-endpoints.md");
  const talisman = read(".well-known/uaix-talisman.txt");
  for (const required of [
    "agent-to-agent long-term memory",
    "meetings",
    "handoffs",
    "coordination",
    "TinyRustLM.com Connect",
    "credentials",
    "consent",
    "provenance",
    "receipts",
    "not MiniModel",
  ]) {
    assert.match(memory + talisman, new RegExp(required.replaceAll(".", "\\."), "i"));
  }
});

test("binds visible and machine-readable release identity", () => {
  const release = readJson(".well-known/site-release.json");
  const history = readJson(".well-known/site-release-history.json");
  const html = read("index.html");
  const updates = read("updates/index.html");
  assert.equal(release.release_id, "minimodel.org@1.1.1");
  assert.equal(release.release_date, "2026-08-29");
  assert.equal(history.current_release_id, release.release_id);
  assert.match(html, /Version 1\.1\.1/);
  assert.match(updates, /1\.1\.1 - 2026-08-29/);
});

test("keeps retired native code and duplicate TinyRustLM authority out of current source", () => {
  for (const retiredPath of ["Cargo.toml", "Cargo.lock"]) {
    assert.equal(
      fs.existsSync(path.join(repositoryRoot, retiredPath)),
      false,
      `${retiredPath} must remain available only through Git history`,
    );
  }

  for (const retiredRoot of ["crates", "artifacts/demo", "tests/fixtures/native-v0"]) {
    const absoluteRoot = path.join(repositoryRoot, retiredRoot);
    const files = fs.existsSync(absoluteRoot)
      ? fs.readdirSync(absoluteRoot, { recursive: true, withFileTypes: true })
          .filter((entry) => entry.isFile())
      : [];
    assert.deepEqual(files, [], `${retiredRoot} must contain no current files`);
  }

  const taboo = read(".uai/taboo.uai");
  for (const required of [
    "more than one authoritative TinyRustLM.com source checkout",
    "any second writable TinyRustLM.com checkout",
    "automated gates must reject pointers to duplicate, retired, quarantined",
    "Historical source belongs only in Git history",
    "Any committed change to a declared public artifact invalidates deployed-current status",
  ]) {
    assert.match(taboo, new RegExp(required.replaceAll(".", "\\."), "i"));
  }

  const roadmap = read("ROADMAP.md");
  assert.match(roadmap, /obsolete pre-SLM2 native CLI.*deleted/s);
  assert.match(roadmap, /only through Git history/);
  assert.doesNotMatch(roadmap, /current native MiniModel CLI predates/i);
});

test("contains no current instruction-file acquisition or remote inference fallback", () => {
  const current = [
    read("index.html"),
    read("ROADMAP.md"),
    read("llms.txt"),
    read("llms-full.txt"),
    read("docs/acquisition-contract.md"),
    read("docs/peer-companion.md"),
  ].join("\n");
  assert.match(current, /no invisible peer rotation|There is no invisible peer rotation/i);
  assert.match(current, /no instruction-file|instruction-file download/i);
  assert.match(current, /remote inference/i);
  assert.doesNotMatch(current, /download (?:a )?(?:json|txt|script|powershell) handoff/i);
});
