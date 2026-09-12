"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
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

const TEST_NOW = new Date("2026-09-04T21:10:00Z");
const utc = (offsetMilliseconds) =>
  new Date(TEST_NOW.getTime() + offsetMilliseconds).toISOString().replace(".000Z", "Z");
const PEER_KEY_ID = sha("9");
const PEER_PUBLIC_KEY = Buffer.alloc(32, 1).toString("base64url");
const PEER_SIGNATURE = Buffer.alloc(64, 2).toString("base64url");

function validEntry(options = {}) {
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
    browser_admission: {
      route: "/catalog/browser-admissions/qwen3-0.6b.browser-composition-admission.json",
      bytes: 4096,
      sha256: sha("f"),
    },
    profiles: {
      conversion: "tinyrustlm.converter.v1",
      quantization: "tinyrustlm.mixed-q8.v1",
      runtime: "tinyrustlm.browser.v1",
    },
    qualification: {
      status: "qualified",
      receipt_id: sha("d"),
      receipt_route: "/catalog/qualification-receipts/qwen3-0.6b.json",
      receipt_bytes: 4096,
      receipt_sha256: sha("d"),
      numerical_status: "passed",
      browser_status: "passed",
      quality_status: "qualified",
    },
    peer_announcement_ids: options.peerIds || [],
  };
}

function validCatalog(entries = []) {
  return {
    schema: catalogApi.CATALOG_SCHEMA,
    catalog_id: "minimodel.org/prelaunch",
    revision: 1,
    generated_at_utc: utc(-60_000),
    valid_until_utc: utc(60 * 60_000),
    project_server_serves_model_bytes: false,
    project_server_proxies_model_bytes: false,
    project_server_runs_inference: false,
    qualification_policy: "exact-current-evidence-required",
    qualification_receipt_schema: catalogApi.QUALIFICATION_RECEIPT_SCHEMA,
    browser_admission_schema: catalogApi.BROWSER_ADMISSION_SCHEMA,
    peer_verification_keys: [{
      key_id: PEER_KEY_ID,
      algorithm: "Ed25519",
      public_key_base64url: PEER_PUBLIC_KEY,
      not_before_utc: utc(-2 * 60 * 60_000),
      not_after_utc: utc(2 * 60 * 60_000),
    }],
    revoked_qualification_receipt_ids: [],
    supersessions: [],
    entries,
  };
}

function validPeerFeed(announcements = []) {
  return {
    schema: catalogApi.PEER_SCHEMA,
    feed_id: "minimodel.org/prelaunch-peers",
    revision: 1,
    generated_at_utc: utc(-60_000),
    valid_until_utc: utc(60 * 60_000),
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
    peer_base_url: "https://8.8.8.8/",
    protocol_profile: catalogApi.DIRECT_COMPOSITION_PROTOCOL_PROFILE,
    consent_status: "explicit",
    consent_receipt_sha256: sha("6"),
    outside_proof_status: "current-passed",
    outside_proof_receipt_sha256: sha("7"),
    issued_at_utc: utc(-60_000),
    expires_utc: utc(30 * 60_000),
    verification_key_id: PEER_KEY_ID,
    signature: PEER_SIGNATURE,
  };
}

function catalogWithPeer() {
  return validCatalog([validEntry({ peerIds: ["peer/qwen3/lane-1"] })]);
}

function validateCatalogFixture(value) {
  return catalogApi.validateCatalog(value, { now: TEST_NOW });
}

test("keeps canonical empty source metadata valid only within its declared window", () => {
  const catalogText = read("catalog/compositions.v1.json");
  const peerText = read("catalog/peer-announcements.v1.json");
  const catalog = catalogApi.parseCanonicalJsonText(catalogText);
  const peers = catalogApi.parseCanonicalJsonText(peerText);
  assert.equal(catalogText, `${JSON.stringify(catalog)}\n`);
  assert.equal(peerText, `${JSON.stringify(peers)}\n`);
  assert.deepEqual(Object.keys(catalog), [
    "schema",
    "catalog_id",
    "revision",
    "generated_at_utc",
    "valid_until_utc",
    "project_server_serves_model_bytes",
    "project_server_proxies_model_bytes",
    "project_server_runs_inference",
    "qualification_policy",
    "qualification_receipt_schema",
    "browser_admission_schema",
    "peer_verification_keys",
    "revoked_qualification_receipt_ids",
    "supersessions",
    "entries",
  ]);
  assert.deepEqual(Object.keys(peers), [
    "schema",
    "feed_id",
    "revision",
    "generated_at_utc",
    "valid_until_utc",
    "project_server_serves_model_bytes",
    "project_server_proxies_model_bytes",
    "announcements",
  ]);
  const options = { now: new Date(catalog.generated_at_utc) };
  assert.equal(catalogApi.validateCatalog(catalog, options), catalog);
  assert.deepEqual(catalogApi.validatePeerFeed(peers, catalog, options), []);
  assert.deepEqual(catalogApi.buildCatalogView(catalog, peers, options), []);
  assert.ok(catalog.revision >= 2);
  assert.equal(peers.revision, catalog.revision);
  assert.equal(peers.generated_at_utc, catalog.generated_at_utc);
  assert.equal(peers.valid_until_utc, catalog.valid_until_utc);
  assert.throws(() => catalogApi.validatePeerFeed(peers, catalog, {
    now: new Date(catalog.valid_until_utc),
  }), /stale/);
  assert.equal(
    Date.parse(catalog.valid_until_utc) - Date.parse(catalog.generated_at_utc),
    24 * 60 * 60 * 1000,
  );
  assert.equal(
    catalog.browser_admission_schema,
    "tinyrustlm.browser-composition-admission/1",
  );
  assert.equal(
    catalog.qualification_receipt_schema,
    "tinyrustlm.qualification-receipt/1",
  );
  assert.deepEqual(catalog.peer_verification_keys, []);
  assert.deepEqual(catalog.revoked_qualification_receipt_ids, []);
  assert.deepEqual(catalog.supersessions, []);
  assert.equal(catalog.entries.length, 0);
  assert.equal(peers.announcements.length, 0);
});

test("rejects noncanonical bytes and missing revision or validity authority", () => {
  assert.throws(
    () => catalogApi.parseCanonicalJsonText(`${JSON.stringify(validCatalog(), null, 2)}\n`),
    /not canonical/,
  );

  const missingRevision = validCatalog();
  delete missingRevision.revision;
  assert.throws(() => validateCatalogFixture(missingRevision), /unknown or missing/);

  const missingValidity = validPeerFeed();
  delete missingValidity.valid_until_utc;
  assert.throws(
    () => catalogApi.validatePeerFeed(missingValidity, validCatalog(), { now: TEST_NOW }),
    /unknown or missing/,
  );
});

test("rejects stale, future, excessive, and cross-document validity windows", () => {
  const stale = validCatalog();
  stale.generated_at_utc = utc(-25 * 60 * 60_000);
  stale.valid_until_utc = utc(30 * 60_000);
  assert.throws(() => validateCatalogFixture(stale), /stale, future-dated, or has an excessive lifetime/);

  const future = validCatalog();
  future.generated_at_utc = utc(6 * 60_000);
  future.valid_until_utc = utc(60 * 60_000);
  assert.throws(() => validateCatalogFixture(future), /stale, future-dated, or has an excessive lifetime/);

  const excessive = validCatalog();
  excessive.valid_until_utc = utc(25 * 60 * 60_000);
  assert.throws(() => validateCatalogFixture(excessive), /stale, future-dated, or has an excessive lifetime/);

  const peers = validPeerFeed();
  peers.generated_at_utc = utc(-2 * 60_000);
  assert.throws(
    () => catalogApi.validatePeerFeed(peers, validCatalog(), { now: TEST_NOW }),
    /not bounded by the current catalog window/,
  );

  const atExpiry = validCatalog();
  assert.throws(
    () => catalogApi.validateCatalog(atExpiry, {
      now: new Date(atExpiry.valid_until_utc),
    }),
    /stale, future-dated, or has an excessive lifetime/,
  );
});

test("projects a qualified composition to explicit pinned local conversion", () => {
  const [view] = catalogApi.buildCatalogView(
    validCatalog([validEntry()]),
    validPeerFeed(),
    { now: TEST_NOW },
  );
  assert.equal(view.availability, "local-conversion");
  assert.equal(view.availabilityLabel, "Pinned local conversion");
  assert.equal(view.peerCount, 0);
  assert.deepEqual(view.browserAdmission, {
    schema: catalogApi.BROWSER_ADMISSION_SCHEMA,
    route: "/catalog/browser-admissions/qwen3-0.6b.browser-composition-admission.json",
    bytes: 4096,
    sha256: sha("f"),
  });
  assert.ok(Object.isFrozen(view.browserAdmission));
});

test("requires asynchronous signature verification before projecting a direct peer", () => {
  const catalog = catalogWithPeer();
  const peers = validPeerFeed([validAnnouncement()]);
  const [validated] = catalogApi.validatePeerFeed(peers, catalog, { now: TEST_NOW });
  assert.equal(validated.peerUrl, "https://8.8.8.8/");
  assert.throws(
    () => catalogApi.buildCatalogView(catalog, peers, { now: TEST_NOW }),
    /asynchronous Ed25519 verification/,
  );
});

test("admits only the exact browser-consumable direct composition protocol", () => {
  const catalog = catalogWithPeer();
  assert.equal(
    catalogApi.DIRECT_COMPOSITION_PROTOCOL_PROFILE,
    "tinyrustlm.direct-composition-http.v1",
  );

  for (const protocolProfile of [
    "minimodel.direct-piece.v1",
    "tinyrustlm.direct-composition.v1",
    "tinyrustlm.direct-composition-http.v0",
    "tinyrustlm.direct-composition-http.v1 ",
    "tinyrustlm.direct-composition-http.v1/fallback",
  ]) {
    const announcement = validAnnouncement();
    announcement.protocol_profile = protocolProfile;
    assert.throws(
      () => catalogApi.validatePeerFeed(validPeerFeed([announcement]), catalog, {
        now: TEST_NOW,
      }),
      /protocol profile is unsupported/,
      protocolProfile,
    );
  }
});

test("accepts only root HTTPS peer origins and explicit loopback HTTP", () => {
  const catalog = catalogWithPeer();
  const accepted = [
    ["https://8.8.8.8", "https://8.8.8.8/"],
    ["https://8.8.8.8:8443/", "https://8.8.8.8:8443/"],
    ["http://127.0.0.1:80/", "http://127.0.0.1/"],
    ["http://127.0.0.1:8123/", "http://127.0.0.1:8123/"],
    ["http://[::1]:8123/", "http://[::1]:8123/"],
  ];

  for (const [input, expected] of accepted) {
    const announcement = validAnnouncement();
    announcement.peer_base_url = input;
    const [validated] = catalogApi.validatePeerFeed(
      validPeerFeed([announcement]),
      catalog,
      { now: TEST_NOW },
    );
    assert.equal(validated.peerUrl, expected, input);
  }
});

test("rejects peer URLs with endpoints, query, fragment, or credentials", () => {
  const catalog = catalogWithPeer();
  for (const peerBaseUrl of [
    "https://peer.direct-model.net/models/qwen3/",
    "https://peer.direct-model.net//",
    "https://peer.direct-model.net/?lane=one",
    "https://peer.direct-model.net/#lane",
    "https://user:secret@peer.direct-model.net/",
  ]) {
    const announcement = validAnnouncement();
    announcement.peer_base_url = peerBaseUrl;
    assert.throws(
      () => catalogApi.validatePeerFeed(validPeerFeed([announcement]), catalog, {
        now: TEST_NOW,
      }),
      /absolute root HTTP\(S\) origin|credentials or an unexpected endpoint path|encoded or unsafe alias/,
      peerBaseUrl,
    );
  }
});

test("rejects public HTTP and non-explicit loopback HTTP", () => {
  const catalog = catalogWithPeer();
  for (const peerBaseUrl of [
    "http://peer.direct-model.net:8123/",
    "http://localhost/",
    "http://127.0.0.1:0/",
    "http://127.0.0.1:65536/",
    "http://127.0.0.1:08123/",
    "http://[0:0:0:0:0:0:0:1]:8123/",
  ]) {
    const announcement = validAnnouncement();
    announcement.peer_base_url = peerBaseUrl;
    assert.throws(
      () => catalogApi.validatePeerFeed(validPeerFeed([announcement]), catalog, {
        now: TEST_NOW,
      }),
      /HTTP requires an exact loopback IP literal|canonical spelling|port must be an integer/,
      peerBaseUrl,
    );
  }
});

test("rejects local, reserved, and project-service HTTPS authorities", () => {
  const catalog = catalogWithPeer();
  for (const peerBaseUrl of [
    "https://192.168.1.1/",
    "https://203.0.113.1/",
    "https://[2001:db8::1]/",
    "https://peer.local/",
    "https://peer.example.net/",
    "https://tinyrustlm.com/",
    "https://seed.minimodel.org/",
  ]) {
    const announcement = validAnnouncement();
    announcement.peer_base_url = peerBaseUrl;
    assert.throws(
      () => catalogApi.validatePeerFeed(validPeerFeed([announcement]), catalog, {
        now: TEST_NOW,
      }),
      /public and non-reserved|local or reserved DNS|project and service hosts/,
      peerBaseUrl,
    );
  }
});

test("allows declared seed hosting but rejects unknown fields and inference", () => {
  assert.throws(
    () => validateCatalogFixture({ ...validCatalog(), legacy_alias: true }),
    /unknown or missing/,
  );
  assert.doesNotThrow(() => validateCatalogFixture({
    ...validCatalog(), project_server_serves_model_bytes: true,
  }));
  assert.throws(() => validateCatalogFixture({
    ...validCatalog(), project_server_runs_inference: true,
  }), /off project servers/);
});

test("rejects incomplete or drifted six-member composition closure", () => {
  const entry = validEntry();
  entry.composition.members.pop();
  entry.composition.member_count = 5;
  assert.throws(
    () => validateCatalogFixture(validCatalog([entry])),
    /exactly six members/,
  );

  const duplicate = validEntry();
  duplicate.composition.members[5].name = duplicate.composition.members[0].name;
  assert.throws(
    () => validateCatalogFixture(validCatalog([duplicate])),
    /member set is not closed/,
  );
});

test("rejects missing, unsafe, or drifted browser admission descriptors", () => {
  const missing = validEntry();
  delete missing.browser_admission;
  assert.throws(
    () => validateCatalogFixture(validCatalog([missing])),
    /unknown or missing/,
  );

  for (const route of [
    "https://minimodel.org/catalog/admission.json",
    "//other.example/admission.json",
    "/catalog/../admission.json",
  ]) {
    const unsafe = validEntry();
    unsafe.browser_admission.route = route;
    assert.throws(
      () => validateCatalogFixture(validCatalog([unsafe])),
      /safe same-origin route/,
    );
  }

  const oversized = validEntry();
  oversized.browser_admission.bytes = catalogApi.MAX_BROWSER_ADMISSION_BYTES + 1;
  assert.throws(
    () => validateCatalogFixture(validCatalog([oversized])),
    /positive bounded safe integer/,
  );

  const unprefixed = validEntry();
  unprefixed.browser_admission.sha256 = "f".repeat(64);
  assert.throws(
    () => validateCatalogFixture(validCatalog([unprefixed])),
    /prefixed SHA-256/,
  );

  const unknown = validEntry();
  unknown.browser_admission.media_type = "application/json";
  assert.throws(
    () => validateCatalogFixture(validCatalog([unknown])),
    /unknown or missing/,
  );

  const driftedSchema = validCatalog([validEntry()]);
  driftedSchema.browser_admission_schema = "tinyrustlm.browser-composition-admission/2";
  assert.throws(
    () => validateCatalogFixture(driftedSchema),
    /browser admission schema is unsupported/,
  );
});

test("rejects an unqualified catalog entry", () => {
  const entry = validEntry();
  entry.qualification.browser_status = "untested";
  assert.throws(
    () => validateCatalogFixture(validCatalog([entry])),
    /qualification is incomplete/,
  );
});

test("rejects expired and project-hosted peer announcements", () => {
  const catalog = catalogWithPeer();
  const expired = validAnnouncement();
  expired.expires_utc = utc(-2 * 60_000);
  assert.throws(
    () => catalogApi.validatePeerFeed(validPeerFeed([expired]), catalog, {
      now: TEST_NOW,
    }),
    /expired/,
  );

  const hosted = validAnnouncement();
  hosted.peer_base_url = "https://tinyrustlm.com/";
  assert.throws(
    () => catalogApi.validatePeerFeed(validPeerFeed([hosted]), catalog, {
      now: TEST_NOW,
    }),
    /non-project peer|project and service hosts/,
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
  const normalizedCatalogScript = read("catalog.js").replaceAll("\r\n", "\n");
  assert.match(normalizedCatalogScript, /cache: "no-store"/);
  assert.match(normalizedCatalogScript, /credentials: "omit"/);
  assert.match(normalizedCatalogScript, /redirect: "error"/);
  const catalogScriptSha256 = crypto.createHash("sha256")
    .update(normalizedCatalogScript, "utf8")
    .digest("hex");
  const cacheIdentity = /catalog\.js\?v=(sha256-[a-f0-9]{64})/.exec(html);
  assert.ok(cacheIdentity, "catalog.js must have a content-bound cache identity");
  assert.equal(cacheIdentity[1], `sha256-${catalogScriptSha256}`);
  assert.match(apache, /R=410/);
  assert.match(apache, /models\\\.v0/);
});

test("prevents stale authority responses without disabling caching for other catalog files", () => {
  const apache = read("catalog/.htaccess").replaceAll("\r\n", "\n");
  const authorityPolicy = [
    "  <FilesMatch \"^(compositions\\.v1\\.json|peer-announcements\\.v1\\.json)$\">",
    "    Header always set Cache-Control \"no-store, max-age=0, must-revalidate\"",
    "  </FilesMatch>",
  ].join("\n");

  assert.ok(apache.includes(authorityPolicy));
  assert.equal((apache.match(/Header always set Cache-Control/gu) || []).length, 1);
  assert.doesNotMatch(apache, /FilesMatch[^\n]*(?:browser-admissions|qualification-receipts|\.json\$)/u);
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
  const catalog = readJson("catalog/compositions.v1.json");
  const peerFeed = readJson("catalog/peer-announcements.v1.json");
  const html = read("index.html");
  const updates = read("updates/index.html");
  const boundary = read("docs/catalog-boundary.md");
  const talisman = read(".well-known/uaix-talisman.txt");
  const manifestBuilder = read("tools/build-site-deployment-manifest.js");
  const peerUrlRule = "Public peer URLs require HTTPS and HTTP is loopback-only.";

  assert.equal(release.release_id, "minimodel.org@1.1.2");
  assert.equal(release.version, "1.1.2");
  assert.equal(release.release_date, "2026-08-31");
  assert.equal(release.last_publish_date, "2026-08-29");
  assert.equal(release.status, "prepared");
  assert.equal(history.current_release_id, release.release_id);
  assert.deepEqual(history.releases.map((entry) => entry.release_id), [
    "minimodel.org@1.1.2",
    "minimodel.org@1.1.1",
    "minimodel.org@1.1.0",
    "minimodel.org@1.0.0",
  ]);
  assert.equal(history.releases[0].release_id, release.release_id);
  assert.equal(history.releases[0].last_publish_date, null);
  assert.equal(history.releases[0].status, "prepared");
  assert.equal(history.releases[1].release_id, "minimodel.org@1.1.1");
  assert.equal(history.releases[1].last_publish_date, "2026-08-29");
  assert.deepEqual(catalog.entries, []);
  assert.deepEqual(peerFeed.announcements, []);
  assert.match(html, /Version 1\.1\.2 \| Prepared 2026-08-31/);
  assert.match(updates, /Prepared release - not published/);
  assert.match(updates, /1\.1\.2 - 2026-08-31/);
  assert.match(talisman, /^release_status=prepared$/m);
  assert.match(manifestBuilder, /const version = "1\.1\.2";/);
  assert.match(manifestBuilder, /release_date: "2026-08-31"/);
  for (const surface of [html, updates, boundary, talisman, JSON.stringify(history)]) {
    assert.ok(surface.includes(peerUrlRule), peerUrlRule);
  }
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
