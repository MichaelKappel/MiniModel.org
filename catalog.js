(function attachMiniModelCatalog(globalObject) {
  "use strict";

  const MAX_METADATA_BYTES = 1024 * 1024;
  const CATALOG_SCHEMA = "minimodel.slm2_catalog.v1";
  const PEER_SCHEMA = "minimodel.slm2_peer_announcements.v1";
  const MEMBER_NAMES = Object.freeze([
    "model.slm2",
    "tokenizer.tokenizer2",
    "template.template2",
    "sampling.sampling2",
    "prompt.prompt2",
    "composition.acg2",
  ]);

  function isObject(value) {
    return Boolean(value) && typeof value === "object" && !Array.isArray(value);
  }

  function requireObject(value, label) {
    if (!isObject(value)) {
      throw new Error(`${label} must be an object`);
    }
    return value;
  }

  function requireExactKeys(value, expected, label) {
    const actual = Object.keys(requireObject(value, label)).sort();
    const wanted = [...expected].sort();
    if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
      throw new Error(`${label} has an unknown or missing field`);
    }
  }

  function requireString(value, label, maximum = 512) {
    if (typeof value !== "string" || !value.trim() || value.length > maximum) {
      throw new Error(`${label} must be a bounded non-empty string`);
    }
    return value.trim();
  }

  function requireId(value, label) {
    const normalized = requireString(value, label, 128);
    if (!/^[a-z0-9][a-z0-9._/-]*$/.test(normalized)) {
      throw new Error(`${label} must be lower-case route-safe ASCII`);
    }
    return normalized;
  }

  function requireSha256(value, label) {
    const normalized = requireString(value, label, 71).toLowerCase();
    if (!/^sha256:[a-f0-9]{64}$/.test(normalized)) {
      throw new Error(`${label} must be a prefixed SHA-256 identity`);
    }
    return normalized;
  }

  function requireUtc(value, label) {
    const normalized = requireString(value, label, 20);
    if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(normalized)
        || !Number.isFinite(Date.parse(normalized))) {
      throw new Error(`${label} must be a UTC timestamp with second precision`);
    }
    return normalized;
  }

  function requireLocalRoute(value, label) {
    const normalized = requireString(value, label, 256);
    if (!/^\/[A-Za-z0-9._/-]+$/.test(normalized)
        || normalized.includes("//")
        || normalized.split("/").includes("..")) {
      throw new Error(`${label} must be a safe same-origin route`);
    }
    return normalized;
  }

  function requireProjectBoundary(value, includeInference, label) {
    if (value.project_server_serves_model_bytes !== false
        || value.project_server_proxies_model_bytes !== false
        || (includeInference && value.project_server_runs_inference !== false)) {
      throw new Error(`${label} must keep model bytes and inference off project servers`);
    }
  }

  function validateMember(value, label) {
    requireExactKeys(value, ["name", "bytes", "sha256", "schema"], label);
    const name = requireString(value.name, `${label}.name`, 64);
    if (!MEMBER_NAMES.includes(name)) {
      throw new Error(`${label}.name is not a current SLM2 member`);
    }
    if (!Number.isSafeInteger(value.bytes) || value.bytes <= 0) {
      throw new Error(`${label}.bytes must be a positive safe integer`);
    }
    requireSha256(value.sha256, `${label}.sha256`);
    requireId(value.schema, `${label}.schema`);
    return value;
  }

  function validateCatalogEntry(value, label) {
    requireExactKeys(value, [
      "entry_id",
      "display_name",
      "description",
      "source",
      "license",
      "composition",
      "profiles",
      "qualification",
      "peer_announcement_ids",
    ], label);
    requireId(value.entry_id, `${label}.entry_id`);
    requireString(value.display_name, `${label}.display_name`, 160);
    requireString(value.description, `${label}.description`, 1000);

    requireExactKeys(value.source, ["repository", "revision", "source_lock_sha256"], `${label}.source`);
    if (!/^[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+$/.test(requireString(value.source.repository, `${label}.source.repository`, 160))) {
      throw new Error(`${label}.source.repository must be an owner/model identity`);
    }
    if (!/^[a-f0-9]{40}$/.test(requireString(value.source.revision, `${label}.source.revision`, 40))) {
      throw new Error(`${label}.source.revision must be an immutable 40-character revision`);
    }
    requireSha256(value.source.source_lock_sha256, `${label}.source.source_lock_sha256`);

    requireExactKeys(value.license, ["route", "review_status"], `${label}.license`);
    requireLocalRoute(value.license.route, `${label}.license.route`);
    if (value.license.review_status !== "reviewed") {
      throw new Error(`${label}.license.review_status must be reviewed`);
    }

    requireExactKeys(value.composition, ["composition_id", "member_count", "members"], `${label}.composition`);
    requireSha256(value.composition.composition_id, `${label}.composition.composition_id`);
    if (value.composition.member_count !== MEMBER_NAMES.length
        || !Array.isArray(value.composition.members)
        || value.composition.members.length !== MEMBER_NAMES.length) {
      throw new Error(`${label}.composition must contain exactly six members`);
    }
    value.composition.members.forEach((member, index) => validateMember(member, `${label}.composition.members[${index}]`));
    const memberNames = value.composition.members.map((member) => member.name).sort();
    if (memberNames.some((name, index) => name !== [...MEMBER_NAMES].sort()[index])) {
      throw new Error(`${label}.composition member set is not closed`);
    }

    requireExactKeys(value.profiles, ["conversion", "quantization", "runtime"], `${label}.profiles`);
    requireId(value.profiles.conversion, `${label}.profiles.conversion`);
    requireId(value.profiles.quantization, `${label}.profiles.quantization`);
    requireId(value.profiles.runtime, `${label}.profiles.runtime`);

    requireExactKeys(value.qualification, [
      "status",
      "receipt_id",
      "receipt_sha256",
      "numerical_status",
      "browser_status",
      "quality_status",
    ], `${label}.qualification`);
    if (value.qualification.status !== "qualified"
        || value.qualification.numerical_status !== "passed"
        || value.qualification.browser_status !== "passed"
        || value.qualification.quality_status !== "qualified") {
      throw new Error(`${label}.qualification is incomplete`);
    }
    requireId(value.qualification.receipt_id, `${label}.qualification.receipt_id`);
    requireSha256(value.qualification.receipt_sha256, `${label}.qualification.receipt_sha256`);

    if (!Array.isArray(value.peer_announcement_ids)
        || new Set(value.peer_announcement_ids).size !== value.peer_announcement_ids.length) {
      throw new Error(`${label}.peer_announcement_ids must be a unique array`);
    }
    value.peer_announcement_ids.forEach((item, index) => requireId(item, `${label}.peer_announcement_ids[${index}]`));
    return value;
  }

  function validateCatalog(value) {
    requireExactKeys(value, [
      "schema",
      "catalog_id",
      "generated_at_utc",
      "project_server_serves_model_bytes",
      "project_server_proxies_model_bytes",
      "project_server_runs_inference",
      "qualification_policy",
      "entries",
    ], "catalog");
    if (value.schema !== CATALOG_SCHEMA
        || value.qualification_policy !== "exact-current-evidence-required") {
      throw new Error("catalog schema or qualification policy is unsupported");
    }
    requireId(value.catalog_id, "catalog.catalog_id");
    requireUtc(value.generated_at_utc, "catalog.generated_at_utc");
    requireProjectBoundary(value, true, "catalog");
    if (!Array.isArray(value.entries)) {
      throw new Error("catalog.entries must be an array");
    }
    const ids = new Set();
    const compositions = new Set();
    value.entries.forEach((entry, index) => {
      validateCatalogEntry(entry, `catalog.entries[${index}]`);
      if (ids.has(entry.entry_id) || compositions.has(entry.composition.composition_id)) {
        throw new Error("catalog contains a duplicate entry or composition identity");
      }
      ids.add(entry.entry_id);
      compositions.add(entry.composition.composition_id);
    });
    return value;
  }

  function validatePeerAnnouncement(value, catalogByComposition, now) {
    requireExactKeys(value, [
      "announcement_id",
      "composition_id",
      "piece_set_sha256",
      "peer_base_url",
      "protocol_profile",
      "consent_status",
      "outside_proof_status",
      "expires_utc",
    ], "peer announcement");
    const id = requireId(value.announcement_id, "peer announcement.announcement_id");
    const compositionId = requireSha256(value.composition_id, "peer announcement.composition_id");
    if (!catalogByComposition.has(compositionId)) {
      throw new Error("peer announcement references an unknown composition");
    }
    requireSha256(value.piece_set_sha256, "peer announcement.piece_set_sha256");
    requireId(value.protocol_profile, "peer announcement.protocol_profile");
    if (value.consent_status !== "explicit"
        || value.outside_proof_status !== "current-passed") {
      throw new Error("peer announcement lacks explicit consent or current outside proof");
    }
    let peerUrl;
    try {
      peerUrl = new URL(requireString(value.peer_base_url, "peer announcement.peer_base_url", 512));
    } catch (_error) {
      throw new Error("peer announcement.peer_base_url must be an absolute URL");
    }
    if (!["http:", "https:"].includes(peerUrl.protocol)
        || peerUrl.origin.toLowerCase() === "https://minimodel.org") {
      throw new Error("peer announcement must name a direct non-project peer");
    }
    const expires = requireUtc(value.expires_utc, "peer announcement.expires_utc");
    if (Date.parse(expires) <= now) {
      throw new Error("peer announcement is expired");
    }
    return { id, compositionId, peerUrl: peerUrl.toString(), expires };
  }

  function validatePeerFeed(value, catalog, options = {}) {
    requireExactKeys(value, [
      "schema",
      "feed_id",
      "generated_at_utc",
      "project_server_serves_model_bytes",
      "project_server_proxies_model_bytes",
      "announcements",
    ], "peer feed");
    if (value.schema !== PEER_SCHEMA) {
      throw new Error("peer feed schema is unsupported");
    }
    requireId(value.feed_id, "peer feed.feed_id");
    requireUtc(value.generated_at_utc, "peer feed.generated_at_utc");
    requireProjectBoundary(value, false, "peer feed");
    if (!Array.isArray(value.announcements)) {
      throw new Error("peer feed.announcements must be an array");
    }
    const now = options.now instanceof Date ? options.now.getTime() : Date.now();
    const catalogByComposition = new Map(catalog.entries.map((entry) => [
      entry.composition.composition_id,
      entry,
    ]));
    const ids = new Set();
    return value.announcements.map((announcement) => {
      const validated = validatePeerAnnouncement(announcement, catalogByComposition, now);
      if (ids.has(validated.id)) {
        throw new Error("peer feed contains a duplicate announcement");
      }
      ids.add(validated.id);
      return validated;
    });
  }

  function buildCatalogView(catalogValue, peerFeedValue, options = {}) {
    const catalog = validateCatalog(catalogValue);
    const announcements = validatePeerFeed(peerFeedValue, catalog, options);
    const peersByComposition = new Map();
    announcements.forEach((announcement) => {
      const list = peersByComposition.get(announcement.compositionId) || [];
      list.push(announcement);
      peersByComposition.set(announcement.compositionId, list);
    });
    return catalog.entries.map((entry) => {
      const peers = peersByComposition.get(entry.composition.composition_id) || [];
      return {
        entryId: entry.entry_id,
        displayName: entry.display_name,
        description: entry.description,
        sourceRepository: entry.source.repository,
        sourceRevision: entry.source.revision,
        compositionId: entry.composition.composition_id,
        quantizationProfile: entry.profiles.quantization,
        licenseRoute: entry.license.route,
        availability: peers.length ? "peer" : "local-conversion",
        availabilityLabel: peers.length
          ? `${peers.length} current direct ${peers.length === 1 ? "peer" : "peers"}`
          : "Pinned local conversion",
        peerCount: peers.length,
      };
    });
  }

  function appendFact(documentObject, list, term, value) {
    const dt = documentObject.createElement("dt");
    dt.textContent = term;
    const dd = documentObject.createElement("dd");
    dd.textContent = value;
    list.append(dt, dd);
  }

  function createCatalogCard(documentObject, view) {
    const item = documentObject.createElement("li");
    item.className = "catalog-model";
    item.dataset.availability = view.availability;

    const header = documentObject.createElement("div");
    header.className = "catalog-model-header";
    const identity = documentObject.createElement("div");
    identity.className = "catalog-model-identity";
    const title = documentObject.createElement("h3");
    title.textContent = view.displayName;
    const id = documentObject.createElement("div");
    id.className = "catalog-model-id";
    id.textContent = view.entryId;
    identity.append(title, id);
    const badge = documentObject.createElement("span");
    badge.className = `availability-badge ${view.availability === "peer" ? "active" : ""}`;
    badge.textContent = view.availabilityLabel;
    header.append(identity, badge);

    const description = documentObject.createElement("p");
    description.className = "catalog-model-description";
    description.textContent = view.description;

    const facts = documentObject.createElement("dl");
    facts.className = "catalog-model-facts";
    appendFact(documentObject, facts, "Source", view.sourceRepository);
    appendFact(documentObject, facts, "Revision", view.sourceRevision);
    appendFact(documentObject, facts, "Composition", view.compositionId);
    appendFact(documentObject, facts, "Quantization", view.quantizationProfile);

    const actions = documentObject.createElement("div");
    actions.className = "catalog-model-actions";
    const license = documentObject.createElement("a");
    license.href = view.licenseRoute;
    license.textContent = "License record";
    const open = documentObject.createElement("a");
    open.href = "https://tinyrustlm.com/";
    open.textContent = "Open TinyRustLM";
    actions.append(license, open);
    item.append(header, description, facts, actions);
    return item;
  }

  function renderCatalog(documentObject, views) {
    const list = documentObject.getElementById("modelCatalogList");
    const queryInput = documentObject.getElementById("modelCatalogSearch");
    const filterSelect = documentObject.getElementById("modelCatalogAvailability");
    const count = documentObject.getElementById("modelCatalogVisibleCount");
    if (!list || !queryInput || !filterSelect || !count) {
      throw new Error("catalog UI is incomplete");
    }
    const renderFiltered = () => {
      const query = queryInput.value.trim().toLowerCase();
      const mode = filterSelect.value;
      const filtered = views.filter((view) => {
        const text = [view.displayName, view.entryId, view.description, view.sourceRepository].join(" ").toLowerCase();
        return (!query || text.includes(query)) && (mode === "all" || view.availability === mode);
      });
      list.replaceChildren();
      if (!filtered.length) {
        const empty = documentObject.createElement("li");
        empty.className = "catalog-empty";
        empty.textContent = views.length
          ? "No qualified composition matches this filter."
          : "No SLM2 composition is qualified yet. The catalog remains empty until exact current evidence exists.";
        list.append(empty);
      } else {
        filtered.forEach((view) => list.append(createCatalogCard(documentObject, view)));
      }
      count.textContent = `${filtered.length} ${filtered.length === 1 ? "composition" : "compositions"}`;
    };
    queryInput.addEventListener("input", renderFiltered);
    filterSelect.addEventListener("change", renderFiltered);
    renderFiltered();
  }

  async function fetchJson(url, fetchImplementation) {
    const response = await fetchImplementation(url, {
      method: "GET",
      cache: "no-store",
      credentials: "omit",
      redirect: "error",
    });
    if (!response.ok) {
      throw new Error(`metadata request failed with HTTP ${response.status}`);
    }
    const text = await response.text();
    if (new TextEncoder().encode(text).length > MAX_METADATA_BYTES) {
      throw new Error("metadata response exceeded the byte limit");
    }
    return JSON.parse(text);
  }

  async function loadCatalog(documentObject, fetchImplementation = globalObject.fetch) {
    if (typeof fetchImplementation !== "function") {
      throw new Error("catalog fetch is unavailable");
    }
    const base = new URL(globalObject.location ? globalObject.location.href : "https://minimodel.org/");
    const [catalog, peerFeed] = await Promise.all([
      fetchJson(new URL("/catalog/compositions.v1.json", base), fetchImplementation),
      fetchJson(new URL("/catalog/peer-announcements.v1.json", base), fetchImplementation),
    ]);
    const views = buildCatalogView(catalog, peerFeed);
    renderCatalog(documentObject, views);

    const total = documentObject.getElementById("modelCatalogTotal");
    const peers = documentObject.getElementById("modelCatalogActive");
    const local = documentObject.getElementById("modelCatalogReady");
    const status = documentObject.getElementById("modelCatalogStatus");
    if (total) total.textContent = `${views.length} qualified compositions`;
    if (peers) peers.textContent = `${views.filter((view) => view.availability === "peer").length} with current direct peers`;
    if (local) local.textContent = `${views.filter((view) => view.availability === "local-conversion").length} pinned conversion routes`;
    if (status) {
      status.dataset.state = "ready";
      status.textContent = views.length
        ? "Current metadata loaded. Every action still requires local verification."
        : "Current metadata loaded. Zero compositions are qualified, so no model action is offered.";
    }
    return views;
  }

  const api = Object.freeze({
    CATALOG_SCHEMA,
    PEER_SCHEMA,
    MEMBER_NAMES,
    MAX_METADATA_BYTES,
    validateCatalog,
    validatePeerFeed,
    buildCatalogView,
    renderCatalog,
    loadCatalog,
  });
  globalObject.MiniModelCatalog = api;

  if (globalObject.document) {
    const start = () => loadCatalog(globalObject.document).catch((error) => {
      const status = globalObject.document.getElementById("modelCatalogStatus");
      if (status) {
        status.dataset.state = "error";
        status.textContent = `Catalog unavailable: ${error.message}`;
      }
    });
    if (globalObject.document.readyState === "loading") {
      globalObject.document.addEventListener("DOMContentLoaded", start, { once: true });
    } else {
      start();
    }
  }

  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  }
})(typeof window !== "undefined" ? window : globalThis);
