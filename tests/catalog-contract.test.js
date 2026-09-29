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

function signedFixture(operatorSeed = false) {
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
  if (operatorSeed) {
    Object.assign(announcement, { peer_base_url: "https://minimodel.org/",
      outside_proof_status: "not-required-operator-seed", outside_proof_receipt_sha256: null });
    pair.catalog.project_server_serves_model_bytes = true;
    pair.peerFeed.project_server_serves_model_bytes = true;
  }
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

test("both contracts admit only the exact signed operator seed with null outside-proof identity", async () => {
  for (const api of [site, browser]) {
    const pair = signedFixture(true);
    const views = await api.buildVerifiedCatalogView(pair.catalog, pair.peerFeed, options);
    assert.equal(views[0].peerCount, 1);
    assert.equal(pair.peerFeed.announcements[0].outside_proof_receipt_sha256, null);
    Object.assign(pair.peerFeed.announcements[0], { peer_base_url: "https://8.8.8.8/",
      outside_proof_status: "current-passed", outside_proof_receipt_sha256: sha("7") });
    await assert.rejects(api.buildVerifiedCatalogView(pair.catalog, pair.peerFeed, options), /signature/);
  }
  for (const change of [
    { peer_base_url: "https://8.8.8.8/" }, { peer_base_url: "http://127.0.0.1:4321/" },
    { peer_base_url: "https://minimodel.org.evil.example/" }, { peer_base_url: "https://user@minimodel.org/" },
    { peer_base_url: "https://minimodel.org./" }, { peer_base_url: "http://minimodel.org/" },
    { peer_base_url: "https://minimodel.org:444/" }, { peer_base_url: "https://minimodel.org/path" },
    { outside_proof_status: "current-passed", outside_proof_receipt_sha256: sha("7") },
    { outside_proof_receipt_sha256: sha("7") }, { outside_proof_receipt_sha256: undefined },
    { outside_proof_receipt_sha256: "null" }, { outside_proof_receipt_sha256: 1 },
    { outside_proof_status: "unknown" }
  ]) {
    const pair = signedFixture(true);
    Object.assign(pair.peerFeed.announcements[0], change);
    rejectBoth(pair, JSON.stringify(change));
  }
  const absent = signedFixture(true);
  delete absent.peerFeed.announcements[0].outside_proof_receipt_sha256;
  rejectBoth(absent, "missing required null field");
});

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

test("both catalog contracts preserve supplementary Unicode from bytes through verified views", async () => {
  for (const description of ["e\u0301", "\u{10000}", "\u{1f680}", "\u{1f469}\u200d\u{1f4bb}", "\u{20000}", "\u{10ffff}"]) {
    const pair = signedFixture();
    pair.catalog.entries[0].description = description;
    for (const api of [site, browser]) {
      const text = canonical(pair.catalog);
      const parsed = api.parseCanonicalJsonText(Buffer.from(text).toString("utf8"));
      assert.equal(canonical(parsed), text);
      const views = await api.buildVerifiedCatalogView(parsed, pair.peerFeed, options);
      assert.equal(views[0].description, description);
    }
  }
});

test("both catalog contracts reject lone Unicode surrogates in nested values and keys", () => {
  for (const value of ["\ud800", "\udbff", "\udc00", "\udfff", "\ud800x", "\ud800\ud800", "\udc00\ud800"]) {
    for (const api of [site, browser]) {
      for (const document of [{ key: [value] }, { [value]: 1 }]) {
        assert.throws(() => api.parseCanonicalJsonText(canonical(document)));
      }
      const pair = signedFixture();
      pair.catalog.entries[0].display_name = value;
      assert.throws(() => api.validateCatalog(pair.catalog, options));
    }
  }
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
    pair.peerFeed.announcements[0].outside_proof_status = normalized === "https://minimodel.org/"
      ? "not-required-operator-seed" : "current-passed";
    pair.peerFeed.announcements[0].outside_proof_receipt_sha256 = normalized === "https://minimodel.org/" ? null : sha("7");
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
    return new Response(canonical(url.pathname.includes("peer-announcements") ? pair.peerFeed : pair.catalog));
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
  const fetch = async (url) => new Response(canonical(url.pathname.includes("peer-announcements") ? pair.peerFeed : pair.catalog));
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
  const fetch = async (url) => new Response(canonical(url.pathname.includes("peer-announcements") ? pair.peerFeed : pair.catalog));
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
  const rejected = assert.rejects(old, /superseded/);
  const fetch = async (url) => new Response(canonical(url.pathname.includes("peer-announcements") ? pair.peerFeed : pair.catalog));
  await Promise.resolve();
  document.getElementById("modelCatalogAvailability").value = "all";
  await site.loadCatalog(document, fetch, options);
  rejectOld(new Error("old request failed"));
  await rejected;
  assert.equal(document.getElementById("modelCatalogStatus").dataset.state, "ready");
  assert.equal(document.getElementById("modelCatalogAvailability").disabled, false);
  assert.equal(typeof document.getElementById("modelCatalogAvailability").onchange, "function");
});

test("catalog deadline settles even when fetch ignores cancellation", async () => {
  const document = documentFixture();
  const pair = emptyPair();
  let release;
  const pending = new Promise(resolve => { release = resolve; });
  const operation = site.loadCatalog(document, async url => {
    await pending;
    return new Response(canonical(url.pathname.includes("peer-announcements") ? pair.peerFeed : pair.catalog));
  }, { ...options, timeoutMilliseconds: 10 }).then(value => ({ value }), error => ({ error }));
  let timeout;
  try {
    const outcome = await Promise.race([operation, new Promise(resolve => {
      timeout = setTimeout(() => resolve(null), 250);
    })]);
    assert.ok(outcome, "catalog deadline did not settle the original operation");
    assert.match(outcome.error?.message || "", /timed out/);
    assert.equal(document.getElementById("modelCatalogStatus").dataset.state, "error");
  } finally {
    clearTimeout(timeout);
    release();
    await operation;
  }
});

test("catalog rejects advertised oversized responses before consuming their body", async () => {
  const pair = emptyPair();
  let reads = 0;
  const response = url => ({ ok: true, status: 200,
    headers: new Headers({ "content-length": String(site.MAX_METADATA_BYTES + 1) }),
    body: { getReader() { reads++; throw new Error("body should not be read"); }, cancel: async () => {} },
    text: async () => { reads++; return canonical(url.pathname.includes("peer-announcements") ? pair.peerFeed : pair.catalog); }
  });
  await assert.rejects(site.loadCatalog(documentFixture(), async url => response(url), options), /byte limit/);
  assert.equal(reads, 0);
});

test("catalog validates deadlines and caller signals before starting requests", async () => {
  for (const timeoutMilliseconds of [null, 0, -1, 1.5, NaN, Infinity, "10", 60001]) {
    let requests = 0;
    await assert.rejects(site.loadCatalog(documentFixture(), () => { requests++; }, { ...options, timeoutMilliseconds }), /timeout/);
    assert.equal(requests, 0);
  }
  for (const signal of [null, {}, { aborted: false }]) {
    await assert.rejects(site.loadCatalog(documentFixture(), () => assert.fail("request started"), { ...options, signal }), /signal/);
  }
  const controller = new AbortController(); const reason = new Error("explicit caller stop");
  controller.abort(reason);
  await assert.rejects(site.loadCatalog(documentFixture(), () => assert.fail("request started"),
    { ...options, signal: controller.signal }), error => error === reason);
});

test("catalog bounds streamed bytes/chunks and cancels both readers on failure", async t => {
  for (const mode of ["too-large", "too-many", "invalid-utf8", "missing-stream", "pending-read", "cancel-fails"]) {
    await t.test(mode, async () => {
      let reads = 0; let cancels = 0; let releases = 0;
      const signals = [];
      const fetch = async (_url, settings) => {
        signals.push(settings.signal);
        return { ok: true, status: 200, headers: new Headers(),
          body: mode === "missing-stream" ? null : { getReader: () => ({
            read: async () => {
              reads++;
              if (mode === "pending-read") return new Promise(() => {});
              return { done: false, value: mode === "too-many" ? new Uint8Array(0) :
                mode === "invalid-utf8" ? new Uint8Array([0xff]) : new Uint8Array(site.MAX_METADATA_BYTES + 1) };
            },
            cancel: () => { cancels++; if (mode === "cancel-fails") throw new Error("cancel failed"); return Promise.resolve(); },
            releaseLock: () => { releases++; }
          }) }
        };
      };
      await assert.rejects(site.loadCatalog(documentFixture(), fetch, { ...options, timeoutMilliseconds: 100 }),
        /byte\/chunk limit|encoded data|stream is unavailable|timed out/);
      await new Promise(resolve => setImmediate(resolve));
      assert.ok(signals.every(signal => signal.aborted));
      assert.equal(releases, mode === "missing-stream" ? 0 : 2);
      assert.equal(cancels, mode === "missing-stream" ? 0 : 2);
      assert.ok(reads <= (mode === "too-many" ? 8194 : 2));
    });
  }
});

test("catalog accepts exact byte limit and split UTF-8, rejecting one byte more", async () => {
  for (const extra of [0, 1]) {
    const pair = emptyPair();
    const document = documentFixture();
    const baseline = canonical(pair.catalog);
    // Padding is a rejected schema field, so exact-bound reading is established before validation.
    const bytes = Buffer.from(baseline.slice(0, -2) + ',"padding":"' + 'x'.repeat(site.MAX_METADATA_BYTES - Buffer.byteLength(baseline) - 13 + extra) + '"}\n');
    assert.equal(bytes.length, site.MAX_METADATA_BYTES + extra);
    await assert.rejects(site.loadCatalog(document, async url => new Response(
      url.pathname.includes("peer-announcements") ? canonical(pair.peerFeed) : bytes), options),
      extra ? /byte\/chunk limit/ : /unknown or missing field/);
  }
  const pair = signedFixture();
  pair.catalog.entries[0].description = "R\u00e9sum\u00e9 \u6f22\u5b57";
  const views = await site.loadCatalog(documentFixture(), async url => {
    const bytes = Buffer.from(canonical(url.pathname.includes("peer-announcements") ? pair.peerFeed : pair.catalog));
    let index = 0;
    return new Response(new ReadableStream({ pull(controller) {
      if (index === bytes.length) controller.close(); else controller.enqueue(bytes.subarray(index, ++index));
    } }));
  }, options);
  assert.equal(views[0].description, pair.catalog.entries[0].description);
});

test("cancellation stops crypto observation before later verification or rendering", async () => {
  for (const stage of ["importKey", "verify"]) {
    const pair = signedFixture(); const controller = new AbortController(); const reason = new Error("stop crypto");
    const document = documentFixture(); let verifyCalls = 0; let release;
    const pending = new Promise(resolve => { release = resolve; });
    let started;
    const start = new Promise(resolve => { started = resolve; });
    const subtle = {
      importKey: () => { if (stage === "importKey") { started(); return pending; } return Promise.resolve({}); },
      verify: () => { verifyCalls++; started(); return pending; }
    };
    const fetch = async url => new Response(canonical(url.pathname.includes("peer-announcements") ? pair.peerFeed : pair.catalog));
    const result = site.loadCatalog(document, fetch, { ...options, crypto: { subtle }, signal: controller.signal });
    const rejection = assert.rejects(result, error => error === reason);
    await start; controller.abort(reason); await rejection;
    release(true); await new Promise(resolve => setImmediate(resolve));
    assert.equal(verifyCalls, stage === "importKey" ? 0 : 1);
    assert.equal(document.getElementById("modelCatalogStatus").dataset.state, "error");
  }
});

test("cancelled fetch disposes late responses and observes failing body cleanup", async () => {
  const controller = new AbortController(); const reason = new Error("stop waiting for headers");
  const pending = []; let cancelled = 0;
  const result = site.loadCatalog(documentFixture(), () => new Promise(resolve => pending.push(resolve)),
    { ...options, signal: controller.signal });
  const rejection = assert.rejects(result, error => error === reason);
  await Promise.resolve(); assert.equal(pending.length, 2);
  controller.abort(reason); await rejection;
  for (const release of pending) release({ body: { cancel() { cancelled++; return Promise.reject(new Error("late cleanup failed")); } } });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(cancelled, 2);
});
