"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const site = require("../catalog.js");
// Test-only read of the named canonical consumer, never a copied implementation or runtime import.
const browser = require("../../TinyRustLM.com/tinyrustlm/app/minimodel-catalog-v1.js");
const { buildEmptyCatalog, prepareEmptyCatalog } = require("../tools/build-empty-catalog.js");

const now = new Date("2026-09-08T02:40:00Z");
const utc = (offset) => new Date(now.getTime() + offset).toISOString().replace(".000Z", "Z");
const options = { now, crypto: crypto.webcrypto };
const parameters = { revision: 3, generatedAtUtc: utc(0), validUntilUtc: utc(86_400_000) };
const sha = (value) => `sha256:${value.repeat(64)}`;
const canonical = (value) => `${JSON.stringify(value)}\n`;

function emptyPair() {
  return buildEmptyCatalog(parameters, options);
}

function signedFixture() {
  // Synthetic metadata and ephemeral test keys only. No real qualification or outside proof.
  const pair = emptyPair();
  const { publicKey, privateKey } = crypto.generateKeyPairSync("ed25519");
  pair.catalog.peer_verification_keys.push({
    key_id: sha("9"), algorithm: "Ed25519",
    public_key_base64url: publicKey.export({ format: "jwk" }).x,
    not_before_utc: utc(-60_000), not_after_utc: utc(86_400_000),
  });
  pair.catalog.entries.push({
    entry_id: "fixture/only", display_name: "Test fixture only",
    description: "Synthetic contract metadata, not qualification evidence.",
    source: { repository: "fixture/only", revision: "a".repeat(40), source_lock_sha256: sha("b") },
    license: { route: "/test-only/license.txt", review_status: "reviewed" },
    composition: { composition_id: sha("c"), member_count: 6,
      members: site.MEMBER_NAMES.map((name, index) => ({
        name, bytes: index + 1, sha256: sha(String(index + 1)), schema: `fixture.member.${index + 1}`,
      })) },
    browser_admission: { route: "/test-only/admission.json", bytes: 4096, sha256: sha("f") },
    profiles: { conversion: "fixture.v1", quantization: "fixture.v1", runtime: "fixture.v1" },
    qualification: { status: "qualified", receipt_id: sha("d"),
      receipt_route: "/test-only/receipt.json", receipt_bytes: 4096, receipt_sha256: sha("d"),
      numerical_status: "passed", browser_status: "passed", quality_status: "qualified" },
    peer_announcement_ids: ["fixture/peer"],
  });
  const announcement = {
    announcement_id: "fixture/peer", composition_id: sha("c"), piece_set_sha256: sha("e"),
    peer_base_url: "https://8.8.8.8/",
    protocol_profile: site.DIRECT_COMPOSITION_PROTOCOL_PROFILE,
    consent_status: "explicit", consent_receipt_sha256: sha("6"),
    outside_proof_status: "current-passed", outside_proof_receipt_sha256: sha("7"),
    issued_at_utc: utc(0), expires_utc: utc(3_600_000), verification_key_id: sha("9"),
    signature: "",
  };
  announcement.signature = crypto.sign(null,
    browser.peerAnnouncementSigningBytes(announcement), privateKey).toString("base64url");
  pair.peerFeed.announcements.push(announcement);
  return pair;
}

function rejectBoth(pair, label) {
  for (const api of [site, browser]) {
    assert.throws(() => api.validatePeerFeed(pair.peerFeed, pair.catalog, options), undefined, label);
  }
}

test("generator and checked-in empty bytes agree with both contracts at issuance, not after expiry", async () => {
  for (const pair of [emptyPair(), {
    catalog: site.parseCanonicalJsonText(fs.readFileSync(path.resolve(__dirname, "../catalog/compositions.v1.json"), "utf8")),
    peerFeed: site.parseCanonicalJsonText(fs.readFileSync(path.resolve(__dirname, "../catalog/peer-announcements.v1.json"), "utf8")),
  }]) {
    const atIssuance = { now: new Date(pair.catalog.generated_at_utc) };
    const expected = buildEmptyCatalog({
      revision: pair.catalog.revision, generatedAtUtc: pair.catalog.generated_at_utc,
      validUntilUtc: pair.catalog.valid_until_utc,
    }, atIssuance);
    assert.deepEqual(pair, expected);
    for (const api of [site, browser]) {
      const catalog = api.parseCanonicalJsonText(canonical(pair.catalog));
      const peerFeed = api.parseCanonicalJsonText(canonical(pair.peerFeed));
      assert.deepEqual(await api.buildVerifiedCatalogView(catalog, peerFeed, atIssuance), []);
      assert.throws(() => api.validatePeerFeed(peerFeed, catalog, {
        now: new Date(catalog.valid_until_utc),
      }), /stale/);
    }
  }
});

test("empty generator enforces explicit fresh, bounded, monotonic revision and time", () => {
  const previous = buildEmptyCatalog({
    revision: 2, generatedAtUtc: utc(-86_400_000), validUntilUtc: utc(0),
  }, { now: new Date(utc(-86_400_000)) });
  assert.deepEqual(prepareEmptyCatalog(previous.catalog, previous.peerFeed, parameters, options), emptyPair());
  for (const change of [
    { revision: 2 }, { revision: 1 }, { revision: 0 }, { revision: "3" },
    { revision: Number.MAX_SAFE_INTEGER + 1 }, { generatedAtUtc: utc(-86_400_000) },
    { generatedAtUtc: utc(360_000) }, { validUntilUtc: utc(0) },
    { validUntilUtc: utc(86_400_001) }, { generatedAtUtc: "2026-02-30T00:00:00Z" },
  ]) {
    assert.throws(() => prepareEmptyCatalog(previous.catalog, previous.peerFeed,
      { ...parameters, ...change }, options));
  }
  previous.peerFeed.revision = 4;
  assert.throws(() => prepareEmptyCatalog(previous.catalog, previous.peerFeed, parameters, options), /newer revision/);
});

test("empty generator cannot erase evidence, authorized keys, or lifecycle history", () => {
  const next = { ...parameters, revision: 4, generatedAtUtc: utc(1000) };
  const signed = signedFixture();
  assert.throws(() => prepareEmptyCatalog(signed.catalog, signed.peerFeed, next, options), /cannot discard/);
  for (const field of ["peer_verification_keys", "revoked_qualification_receipt_ids", "supersessions"]) {
    const pair = emptyPair();
    pair.catalog[field] = field === "peer_verification_keys" ? signed.catalog[field]
      : field === "revoked_qualification_receipt_ids" ? [sha("d")]
        : [{ superseded_composition_id: sha("c"), replacement_composition_id: sha("b"), effective_at_utc: utc(0) }];
    assert.throws(() => prepareEmptyCatalog(pair.catalog, pair.peerFeed, next, options), /cannot discard/);
  }
});

test("both validators require the current qualification descriptor and lifecycle authority", () => {
  const changes = [
    (c) => { delete c.qualification_receipt_schema; },
    (c) => { c.qualification_receipt_schema = "tinyrustlm.qualification-receipt/2"; },
    (c) => { delete c.revoked_qualification_receipt_ids; },
    (c) => { delete c.supersessions; },
    (c) => { c.revoked_qualification_receipt_ids = [sha("d")]; },
    (c) => { c.revoked_qualification_receipt_ids = [sha("b"), sha("b")]; },
    (c) => { c.supersessions = [{ superseded_composition_id: sha("c"), replacement_composition_id: sha("b"), effective_at_utc: utc(0) }]; },
    (c) => { c.supersessions = [{ superseded_composition_id: sha("b"), replacement_composition_id: sha("b"), effective_at_utc: utc(0) }]; },
    (c) => { c.entries[0].composition.members.reverse(); },
  ];
  for (const field of ["receipt_id", "receipt_route", "receipt_bytes", "receipt_sha256"]) {
    changes.push((c) => { delete c.entries[0].qualification[field]; });
  }
  for (const route of ["https://minimodel.org/receipt.json", "//elsewhere/receipt.json", "/../receipt.json", "/%2e%2e/receipt.json", "/receipt.json?x=1"]) {
    changes.push((c) => { c.entries[0].qualification.receipt_route = route; });
  }
  for (const size of [0, 1, -1, 1.5, "4096", 128 * 1024 + 1]) {
    changes.push((c) => { c.entries[0].qualification.receipt_bytes = size; });
  }
  for (const digest of ["d".repeat(64), sha("D"), "sha256:bad"]) {
    changes.push((c) => { c.entries[0].qualification.receipt_sha256 = digest; });
  }
  for (const mutate of changes) {
    const pair = signedFixture();
    mutate(pair.catalog);
    rejectBoth(pair, mutate.toString());
  }
});

test("both validators reject missing, stale, or unbound signature authority", () => {
  const changes = [
    (c) => { delete c.revision; }, (c) => { c.revision = 0; },
    (c) => { c.generated_at_utc = utc(-90_000_000); },
    (c) => { c.generated_at_utc = utc(360_000); },
    (c) => { c.valid_until_utc = utc(86_401_000); },
    (c) => { c.peer_verification_keys = []; },
    (c) => { c.peer_verification_keys.push(c.peer_verification_keys[0]); },
    (c) => { c.peer_verification_keys[0].algorithm = "RSA"; },
    (c) => { c.peer_verification_keys[0].public_key_base64url += "="; },
    (c) => { c.peer_verification_keys[0].not_after_utc = utc(0); },
    (c) => { c.peer_verification_keys[0].not_before_utc = utc(1000); },
    (c) => { c.peer_verification_keys[0].not_after_utc = utc(1000); },
    (c, p) => { delete p.revision; }, (c, p) => { delete p.valid_until_utc; },
    (c, p) => { p.generated_at_utc = utc(-1000); },
    (c, p) => { p.valid_until_utc = utc(3_600_000); p.announcements[0].expires_utc = utc(3_601_000); },
    (c, p) => { p.announcements[0].issued_at_utc = utc(1000); },
    (c, p) => { delete p.announcements[0].signature; },
    (c, p) => { p.announcements[0].signature += "="; },
    (c, p) => { p.announcements[0].verification_key_id = sha("a"); },
    (c, p) => { p.announcements[0].composition_id = sha("a"); },
    (c, p) => { p.announcements = []; },
    (c) => { c.entries[0].peer_announcement_ids = []; },
  ];
  for (const mutate of changes) {
    const pair = signedFixture();
    mutate(pair.catalog, pair.peerFeed);
    rejectBoth(pair, mutate.toString());
  }
});

test("real test-only Ed25519 signatures interoperate, including normalized peer origins", async () => {
  const pair = signedFixture();
  pair.peerFeed.announcements[0].peer_base_url = "https://8.8.8.8";
  for (const api of [site, browser]) {
    assert.throws(() => api.buildCatalogView(pair.catalog, pair.peerFeed, options), /asynchronous Ed25519/);
    const [view] = await api.buildVerifiedCatalogView(pair.catalog, pair.peerFeed, options);
    assert.equal(view.availability, "peer");
    assert.equal(view.peerCount, 1);
    assert.match(view.availabilityLabel, /reachability not proven/);
    assert.equal(view.qualification.receiptRoute, "/test-only/receipt.json");
    assert.equal(view.qualification.receiptBytes, 4096);
    assert.equal(view.qualification.receiptSha256, sha("d"));
  }
});

test("both contracts admit exact IP origins and reject DNS, aliases, and the application origin", () => {
  const pair = signedFixture();
  for (const [url, normalized] of [
    ["https://minimodel.org:443/", "https://minimodel.org/"],
    ["https://8.8.8.8:443", "https://8.8.8.8/"],
    ["https://[2606:4700:4700::1111]:8443/", "https://[2606:4700:4700::1111]:8443/"],
    ["http://127.0.0.1", "http://127.0.0.1/"],
    ["http://[::1]:80/", "http://[::1]/"],
    ["https://127.0.0.1:8123/", "https://127.0.0.1:8123/"],
  ]) {
    pair.peerFeed.announcements[0].peer_base_url = url;
    const [local] = site.validatePeerFeed(pair.peerFeed, pair.catalog, options);
    const [consumer] = browser.validatePeerFeed(pair.peerFeed, pair.catalog, options);
    assert.equal(local.peerUrl, normalized);
    assert.equal(consumer.peerBaseUrl, normalized);
    for (const api of [site, browser]) {
      if (normalized === "https://minimodel.org/") {
        assert.doesNotThrow(() => api.validatePeerFeed(pair.peerFeed, pair.catalog, {
          ...options, applicationOrigin: normalized,
        }));
        continue;
      }
      assert.throws(() => api.validatePeerFeed(pair.peerFeed, pair.catalog, {
        ...options, applicationOrigin: normalized,
      }), /application origin/);
    }
  }
  for (const url of [
    "https://peer.direct-model.net/", "http://localhost:8123/", "https://localhost/",
    "https://minimodel.org:8443/", "https://seed.minimodel.org/", "http://8.8.8.8:8123/",
    "http://2130706433/", "https://0177.0.0.1/", "http://[0:0:0:0:0:0:0:1]/",
    "https://192.168.1.1/", "https://[2001:db8::1]/", "https://8.8.8.8:0443/",
    "https://8.8.8.8/?lane=1", "https://8.8.8.8/member", "https://8.8.8.8:65536/",
  ]) {
    pair.peerFeed.announcements[0].peer_base_url = url;
    rejectBoth(pair, url);
  }
});

test("unrelated lifecycle history and future supersessions do not revoke a current entry", () => {
  const pair = signedFixture();
  pair.catalog.revoked_qualification_receipt_ids = [sha("b")];
  const edge = { superseded_composition_id: sha("c"), replacement_composition_id: sha("b"), effective_at_utc: utc(1000) };
  pair.catalog.supersessions = [edge];
  for (const receiptBytes of [2, 128 * 1024]) {
    pair.catalog.entries[0].qualification.receipt_bytes = receiptBytes;
    for (const api of [site, browser]) api.validatePeerFeed(pair.peerFeed, pair.catalog, options);
  }
  pair.catalog.supersessions.push({ ...edge });
  rejectBoth(pair, "duplicate supersession");
});

test("every signed identity field is cryptographically bound before peer projection", async () => {
  for (const [field, replacement] of Object.entries({
    announcement_id: "fixture/other", composition_id: sha("b"), piece_set_sha256: sha("b"),
    peer_base_url: "https://8.8.4.4/", consent_receipt_sha256: sha("b"),
    outside_proof_receipt_sha256: sha("b"), issued_at_utc: utc(-1000), expires_utc: utc(2000),
    verification_key_id: sha("b"), signature: Buffer.alloc(64, 2).toString("base64url"),
  })) {
    const pair = signedFixture();
    pair.peerFeed.announcements[0][field] = replacement;
    if (field === "announcement_id") pair.catalog.entries[0].peer_announcement_ids = [replacement];
    if (field === "composition_id") pair.catalog.entries[0].composition.composition_id = replacement;
    if (field === "verification_key_id") pair.catalog.peer_verification_keys[0].key_id = replacement;
    for (const api of [site, browser]) {
      await assert.rejects(api.buildVerifiedCatalogView(pair.catalog, pair.peerFeed, options), /signature/, field);
    }
  }
  const pair = signedFixture();
  pair.catalog.peer_verification_keys[0].public_key_base64url = signedFixture().catalog.peer_verification_keys[0].public_key_base64url;
  for (const api of [site, browser]) {
    await assert.rejects(api.buildVerifiedCatalogView(pair.catalog, pair.peerFeed, options), /signature/);
    await assert.rejects(api.buildVerifiedCatalogView(pair.catalog, pair.peerFeed, { now, crypto: {} }), /unavailable/);
  }
});

test("async verification projects only its owned snapshot, never caller mutations during await", async () => {
  const pair = signedFixture();
  const pending = site.buildVerifiedCatalogView(pair.catalog, pair.peerFeed, options);
  pair.catalog.entries[0].display_name = "mutated";
  pair.catalog.entries[0].qualification.receipt_sha256 = sha("a");
  pair.catalog.revoked_qualification_receipt_ids.push(sha("d"));
  pair.peerFeed.announcements[0].signature = "mutated";
  const [view] = await pending;
  assert.equal(view.displayName, "Test fixture only");
  assert.equal(view.qualification.receiptSha256, sha("d"));
});

function documentFixture() {
  const nodes = new Map();
  const element = () => ({
    children: [], dataset: {}, value: "", textContent: "",
    append(...items) { this.children.push(...items); },
    replaceChildren(...items) { this.children = items; }, addEventListener() {},
  });
  return {
    createElement: element,
    getElementById(id) {
      if (!nodes.has(id)) nodes.set(id, element());
      return nodes.get(id);
    },
  };
}

test("page loader awaits signature verification and never renders a tampered feed", async () => {
  const pair = signedFixture();
  const document = documentFixture();
  document.getElementById("modelCatalogAvailability").value = "all";
  const requested = [];
  const fetch = async (url, settings) => {
    requested.push(url.pathname);
    assert.equal(settings.credentials, "omit");
    assert.equal(settings.cache, "no-store");
    assert.equal(settings.redirect, "error");
    return { ok: true, text: async () => canonical(url.pathname.includes("peer-announcements") ? pair.peerFeed : pair.catalog) };
  };
  const [view] = await site.loadCatalog(document, fetch, options);
  assert.equal(view.availability, "peer");
  assert.equal(document.getElementById("modelCatalogStatus").dataset.state, "ready");
  assert.equal(document.getElementById("modelCatalogList").children.length, 1);
  assert.deepEqual(requested, ["/catalog/compositions.v1.json", "/catalog/peer-announcements.v1.json"]);
  pair.peerFeed.announcements[0].piece_set_sha256 = sha("a");
  const rejectedDocument = documentFixture();
  await assert.rejects(site.loadCatalog(rejectedDocument, fetch, options), /signature/);
  assert.equal(rejectedDocument.getElementById("modelCatalogStatus").dataset.state, "error");
  assert.match(rejectedDocument.getElementById("modelCatalogList").children[0].textContent, /catalog is unavailable/);
});

test("failed reload clears prior actions and callbacks, and a successful retry restores controls", async () => {
  const pair = signedFixture();
  const document = documentFixture();
  const fetch = async (url) => ({ ok: true, text: async () => canonical(url.pathname.includes("peer-announcements") ? pair.peerFeed : pair.catalog) });
  document.getElementById("modelCatalogAvailability").value = "all";
  await site.loadCatalog(document, fetch, options);
  const filter = document.getElementById("modelCatalogAvailability");
  assert.equal(typeof filter.onchange, "function");
  assert.equal(filter.disabled, false);
  await assert.rejects(site.loadCatalog(document, async () => ({ ok: false, status: 503 }), options), /HTTP 503/);
  assert.equal(filter.disabled, true);
  assert.equal(filter.onchange, null);
  assert.equal(document.getElementById("modelCatalogSearch").oninput, null);
  assert.equal(document.getElementById("modelCatalogStatus").dataset.state, "error");
  assert.match(document.getElementById("modelCatalogList").children[0].textContent, /catalog is unavailable/);
  for (const id of ["modelCatalogTotal", "modelCatalogActive", "modelCatalogReady", "modelCatalogVisibleCount"]) {
    assert.equal(document.getElementById(id).textContent, "Unavailable");
  }
  await site.loadCatalog(document, fetch, options);
  assert.equal(filter.disabled, false);
  assert.equal(typeof filter.onchange, "function");
  assert.equal(document.getElementById("modelCatalogStatus").dataset.state, "ready");
});

test("an older load cannot overwrite the current catalog or its failure state", async () => {
  const pair = signedFixture();
  const document = documentFixture();
  let releaseOld;
  const waiting = new Promise((resolve) => { releaseOld = resolve; });
  const fetch = async (url) => ({ ok: true, text: async () => canonical(url.pathname.includes("peer-announcements") ? pair.peerFeed : pair.catalog) });
  const old = site.loadCatalog(document, async (url) => { await waiting; return fetch(url); }, options);
  const rejection = assert.rejects(old, /superseded/);
  await assert.rejects(site.loadCatalog(document, async () => ({ ok: false, status: 503 }), options), /HTTP 503/);
  releaseOld();
  await rejection;
  assert.equal(document.getElementById("modelCatalogStatus").dataset.state, "error");
  assert.equal(document.getElementById("modelCatalogAvailability").disabled, true);
  assert.match(document.getElementById("modelCatalogList").children[0].textContent, /catalog is unavailable/);
});

test("an older failed load cannot clear a newer verified catalog", async () => {
  const pair = signedFixture();
  const document = documentFixture();
  let rejectOld;
  const waiting = new Promise((resolve, reject) => { rejectOld = reject; });
  const old = site.loadCatalog(document, async () => waiting, options);
  const rejected = assert.rejects(old, /old request failed/);
  const fetch = async (url) => ({ ok: true, text: async () => canonical(url.pathname.includes("peer-announcements") ? pair.peerFeed : pair.catalog) });
  document.getElementById("modelCatalogAvailability").value = "all";
  await site.loadCatalog(document, fetch, options);
  rejectOld(new Error("old request failed"));
  await rejected;
  assert.equal(document.getElementById("modelCatalogStatus").dataset.state, "ready");
  assert.equal(document.getElementById("modelCatalogAvailability").disabled, false);
  assert.equal(typeof document.getElementById("modelCatalogAvailability").onchange, "function");
});
