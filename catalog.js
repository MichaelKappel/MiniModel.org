(function attachMiniModelCatalog(globalObject) {
  "use strict";

  const MAX_METADATA_BYTES = 4 * 1024 * 1024;
  const CATALOG_KIND = "minimodel.model_catalog";
  const FEED_KIND = "minimodel.peer_source_announcement_catalog";
  const ANNOUNCEMENT_KIND = "minimodel.peer_source_announcement";

  function isObject(value) {
    return Boolean(value) && typeof value === "object" && !Array.isArray(value);
  }

  function requireObject(value, label) {
    if (!isObject(value)) {
      throw new Error(`${label} must be an object`);
    }
    return value;
  }

  function requireString(value, label) {
    if (typeof value !== "string" || !value.trim()) {
      throw new Error(`${label} must be a non-empty string`);
    }
    return value.trim();
  }

  function requireSafeMetadataBoundary(value, label, requireArtifactRoute) {
    requireObject(value, label);
    if (value.project_server_serves_model_bytes !== false) {
      throw new Error(`${label} must declare project_server_serves_model_bytes=false`);
    }
    if (value.project_server_runs_model_bytes !== false) {
      throw new Error(`${label} must declare project_server_runs_model_bytes=false`);
    }
    if (Object.prototype.hasOwnProperty.call(value, "project_server_proxies_model_bytes") && value.project_server_proxies_model_bytes !== false) {
      throw new Error(`${label} must declare project_server_proxies_model_bytes=false`);
    }
    if (requireArtifactRoute && value.artifact_project_server_url !== null) {
      throw new Error(`${label} must not include a project-hosted artifact URL`);
    }
  }

  function requireSha256(value, label) {
    const normalized = requireString(value, label).toLowerCase();
    if (!/^sha256:[a-f0-9]{64}$/.test(normalized)) {
      throw new Error(`${label} must be a prefixed SHA-256 value`);
    }
    return normalized;
  }

  function requireHuggingFaceModelId(value, label) {
    const modelId = requireString(value, label);
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,95}\/[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(modelId)) {
      throw new Error(`${label} must be an owner/model Hugging Face identifier`);
    }
    return modelId;
  }

  function normalizePeerUrl(value, label, siteOrigin) {
    const raw = requireString(value, label);
    let url;
    try {
      url = new URL(raw);
    } catch (_error) {
      throw new Error(`${label} must be an absolute HTTP URL`);
    }
    if (url.protocol !== "http:" && url.protocol !== "https:") {
      throw new Error(`${label} must use HTTP or HTTPS`);
    }
    if (siteOrigin && url.origin.toLowerCase() === siteOrigin.toLowerCase()) {
      throw new Error(`${label} must not point at the MiniModel project server`);
    }
    url.hash = "";
    url.search = "";
    if (!url.pathname.endsWith("/")) {
      url.pathname += "/";
    }
    return url.toString();
  }

  function validatePeerSource(value, label, siteOrigin) {
    const source = requireObject(value, label);
    requireSafeMetadataBoundary(source, label, false);
    if (source.source_kind !== "known-direct-http-peer-v0") {
      throw new Error(`${label}.source_kind is unsupported`);
    }
    if (source.transport !== "direct-http-piece-server-v0") {
      throw new Error(`${label}.transport is unsupported`);
    }
    if (source.consent_status !== "publisher-declared-consenting-peer") {
      throw new Error(`${label} is missing publisher-declared peer consent`);
    }
    return normalizePeerUrl(source.peer_base_url, `${label}.peer_base_url`, siteOrigin);
  }

  function validateCatalog(catalog, options = {}) {
    const value = requireObject(catalog, "catalog");
    if (value.catalog_kind !== CATALOG_KIND || value.catalog_version !== 0) {
      throw new Error("catalog kind or version is unsupported");
    }
    requireSafeMetadataBoundary(value, "catalog", true);
    if (!Array.isArray(value.entries)) {
      throw new Error("catalog.entries must be an array");
    }
    const seenModelIds = new Set();
    for (const [index, entryValue] of value.entries.entries()) {
      const label = `catalog.entries[${index}]`;
      const entry = requireObject(entryValue, label);
      if (entry.entry_kind !== "minimodel.catalog_entry" || entry.catalog_version !== 0) {
        throw new Error(`${label} kind or version is unsupported`);
      }
      requireSafeMetadataBoundary(entry, label, true);
      if (entry.artifact_kind !== "slm") {
        throw new Error(`${label}.artifact_kind must be slm`);
      }
      const modelId = requireString(entry.model_id, `${label}.model_id`);
      if (seenModelIds.has(modelId)) {
        throw new Error(`catalog has duplicate model_id ${modelId}`);
      }
      seenModelIds.add(modelId);
      requireString(entry.display_name || modelId, `${label}.display_name`);
      if (entry.description !== undefined) {
        requireString(entry.description, `${label}.description`);
      }
      if (entry.source_huggingface_model_id !== undefined) {
        requireHuggingFaceModelId(entry.source_huggingface_model_id, `${label}.source_huggingface_model_id`);
      } else {
        requireString(entry.source_identity, `${label}.source_identity`);
      }
      requireString(entry.model_card_route, `${label}.model_card_route`);
      requireString(entry.manifest_route, `${label}.manifest_route`);
      requireString(entry.peer_offer_route, `${label}.peer_offer_route`);
      requireSha256(entry.artifact_sha256, `${label}.artifact_sha256`);
      requireSha256(entry.peer_offer_sha256, `${label}.peer_offer_sha256`);
      requireSha256(entry.peer_offer_merkle_root_sha256, `${label}.peer_offer_merkle_root_sha256`);
      if (!Number.isSafeInteger(entry.artifact_byte_count) || entry.artifact_byte_count <= 0) {
        throw new Error(`${label}.artifact_byte_count must be a positive integer`);
      }
      const peerSources = entry.peer_sources === undefined ? [] : entry.peer_sources;
      if (!Array.isArray(peerSources)) {
        throw new Error(`${label}.peer_sources must be an array when present`);
      }
      const declaredCount = entry.peer_source_count === undefined ? 0 : entry.peer_source_count;
      if (!Number.isSafeInteger(declaredCount) || declaredCount !== peerSources.length) {
        throw new Error(`${label}.peer_source_count must match peer_sources`);
      }
      peerSources.forEach((source, peerIndex) => {
        validatePeerSource(source, `${label}.peer_sources[${peerIndex}]`, options.siteOrigin || "");
      });
    }
    const routes = value.peer_source_announcement_routes === undefined
      ? []
      : value.peer_source_announcement_routes;
    if (!Array.isArray(routes)) {
      throw new Error("catalog.peer_source_announcement_routes must be an array");
    }
    for (const [index, routeValue] of routes.entries()) {
      const label = `catalog.peer_source_announcement_routes[${index}]`;
      const route = requireObject(routeValue, label);
      requireString(route.route, `${label}.route`);
      requireSha256(route.sha256, `${label}.sha256`);
      if (route.route_kind !== FEED_KIND) {
        throw new Error(`${label}.route_kind is unsupported`);
      }
    }
    return value;
  }

  function validateAnnouncement(announcementValue, entriesById, options = {}) {
    const announcement = requireObject(announcementValue, "peer announcement");
    if (announcement.announcement_kind !== ANNOUNCEMENT_KIND || announcement.announcement_version !== 0) {
      throw new Error("peer announcement kind or version is unsupported");
    }
    if (announcement.announcement_status !== "peer-source-available-metadata-only") {
      throw new Error("peer announcement status is unsupported");
    }
    requireSafeMetadataBoundary(announcement, "peer announcement", false);
    if (announcement.artifact_kind !== "slm") {
      throw new Error("peer announcement.artifact_kind must be slm");
    }
    requireSha256(announcement.catalog_entry_sha256, "peer announcement.catalog_entry_sha256");
    const modelId = requireString(announcement.model_id, "peer announcement.model_id");
    const entry = entriesById.get(modelId);
    if (!entry) {
      throw new Error(`peer announcement references unknown model ${modelId}`);
    }
    const bindings = [
      ["artifact_sha256", requireSha256],
      ["peer_offer_sha256", requireSha256],
      ["peer_offer_merkle_root_sha256", requireSha256]
    ];
    for (const [field, normalizer] of bindings) {
      const announced = normalizer(announcement[field], `peer announcement.${field}`);
      const expected = normalizer(entry[field], `catalog entry.${field}`);
      if (announced !== expected) {
        throw new Error(`peer announcement ${field} does not match catalog entry`);
      }
    }
    if (announcement.artifact_byte_count !== entry.artifact_byte_count) {
      throw new Error("peer announcement artifact_byte_count does not match catalog entry");
    }
    if (announcement.peer_offer_chunk_count !== entry.peer_offer_chunk_count) {
      throw new Error("peer announcement peer_offer_chunk_count does not match catalog entry");
    }
    if (announcement.peer_offer_route !== entry.peer_offer_route) {
      throw new Error("peer announcement peer_offer_route does not match catalog entry");
    }
    return {
      modelId,
      peerUrl: validatePeerSource(announcement.peer_source, "peer announcement.peer_source", options.siteOrigin || "")
    };
  }

  function validateAnnouncementFeed(feedValue, catalog, options = {}) {
    const feed = requireObject(feedValue, "peer announcement feed");
    if (feed.announcement_catalog_kind !== FEED_KIND || feed.announcement_catalog_version !== 0) {
      throw new Error("peer announcement feed kind or version is unsupported");
    }
    requireSafeMetadataBoundary(feed, "peer announcement feed", false);
    if (!Array.isArray(feed.announcements)) {
      throw new Error("peer announcement feed.announcements must be an array");
    }
    const entriesById = new Map(catalog.entries.map((entry) => [entry.model_id, entry]));
    return feed.announcements.map((announcement) => validateAnnouncement(announcement, entriesById, options));
  }

  function fallbackDescription(entry) {
    if (entry.source_huggingface_model_id) {
      return `Checksum-bound .slm package converted from Hugging Face source ${entry.source_huggingface_model_id}. Review its model card, license, and quality evidence before local use.`;
    }
    return `Checksum-bound .slm package for ${entry.model_id}, prepared for local verification and consent-based peer sharing.`;
  }

  function qualityBoundary(entry) {
    if (entry.trained_quality_claim === "not-claimed" && String(entry.quality_claim || "").includes("runtime")) {
      return "Runtime smoke only; conversational quality is not claimed";
    }
    if (entry.trained_quality_claim === "not-claimed") {
      return "Trained quality is not claimed";
    }
    return String(entry.trained_quality_claim || entry.quality_claim || "Review model evidence").replace(/-/g, " ");
  }

  function buildCatalogView(catalogValue, validatedAnnouncements = [], options = {}) {
    const catalog = validateCatalog(catalogValue, options);
    const peersByModel = new Map();
    for (const entry of catalog.entries) {
      const peers = new Set();
      for (const [index, source] of (entry.peer_sources || []).entries()) {
        peers.add(validatePeerSource(source, `${entry.model_id}.peer_sources[${index}]`, options.siteOrigin || ""));
      }
      peersByModel.set(entry.model_id, peers);
    }
    for (const announcement of validatedAnnouncements) {
      if (!isObject(announcement) || !peersByModel.has(announcement.modelId)) {
        throw new Error("validated peer announcement references an unknown catalog model");
      }
      peersByModel.get(announcement.modelId).add(requireString(announcement.peerUrl, "validated peer URL"));
    }
    return catalog.entries.map((entry) => {
      const activePeerCount = peersByModel.get(entry.model_id).size;
      return {
        modelId: entry.model_id,
        displayName: entry.display_name || entry.model_id,
        description: entry.description || fallbackDescription(entry),
        sourceModel: entry.source_huggingface_model_id || entry.source_identity || "Not declared",
        sourceKind: entry.source_huggingface_model_id ? "Hugging Face" : (entry.source_kind || "Declared source"),
        sourceHuggingFaceModelId: entry.source_huggingface_model_id || "",
        quantization: entry.slm_quantization || "Not declared",
        artifactByteCount: entry.artifact_byte_count,
        activePeerCount,
        availability: activePeerCount > 0 ? "active" : "share-ready",
        availabilityLabel: activePeerCount > 0
          ? `${activePeerCount} active ${activePeerCount === 1 ? "peer" : "peers"}`
          : "Share-ready, no active peer",
        qualityBoundary: qualityBoundary(entry),
        modelCardRoute: entry.model_card_route || "",
        manifestRoute: entry.manifest_route || "",
        peerOfferRoute: entry.peer_offer_route || ""
      };
    }).sort((left, right) => {
      if (left.activePeerCount !== right.activePeerCount) {
        return right.activePeerCount - left.activePeerCount;
      }
      return left.displayName.localeCompare(right.displayName, "en", { sensitivity: "base" });
    });
  }

  function filterCatalogViews(views, query, availability) {
    const needle = String(query || "").trim().toLowerCase();
    const filter = availability || "all";
    return views.filter((view) => {
      if (filter === "active" && view.availability !== "active") {
        return false;
      }
      if (filter === "share-ready" && view.availability !== "share-ready") {
        return false;
      }
      if (!needle) {
        return true;
      }
      return [view.displayName, view.modelId, view.description, view.sourceModel, view.quantization]
        .some((value) => String(value || "").toLowerCase().includes(needle));
    });
  }

  function formatBytes(value) {
    const bytes = Number(value);
    if (!Number.isFinite(bytes) || bytes < 0) {
      return "Unknown size";
    }
    if (bytes < 1024) {
      return `${bytes} B`;
    }
    const units = ["KiB", "MiB", "GiB", "TiB"];
    let scaled = bytes / 1024;
    let unitIndex = 0;
    while (scaled >= 1024 && unitIndex < units.length - 1) {
      scaled /= 1024;
      unitIndex += 1;
    }
    return `${scaled >= 10 ? scaled.toFixed(1) : scaled.toFixed(2)} ${units[unitIndex]}`;
  }

  async function sha256Prefixed(text, cryptoObject) {
    const cryptoApi = cryptoObject || globalObject.crypto;
    if (!cryptoApi || !cryptoApi.subtle) {
      throw new Error("Web Crypto SHA-256 is unavailable");
    }
    const bytes = new TextEncoder().encode(text);
    const digest = await cryptoApi.subtle.digest("SHA-256", bytes);
    const hex = Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
    return `sha256:${hex}`;
  }

  async function fetchBoundedText(url, fetchFunction) {
    const response = await fetchFunction(url, { cache: "no-store", credentials: "omit" });
    if (!response.ok) {
      throw new Error(`${url.pathname} returned HTTP ${response.status}`);
    }
    const declaredLength = Number(response.headers && response.headers.get("content-length"));
    if (Number.isFinite(declaredLength) && declaredLength > MAX_METADATA_BYTES) {
      throw new Error(`${url.pathname} exceeds the metadata size limit`);
    }
    const text = await response.text();
    if (new TextEncoder().encode(text).length > MAX_METADATA_BYTES) {
      throw new Error(`${url.pathname} exceeds the metadata size limit`);
    }
    return text;
  }

  function createElement(documentObject, tagName, className, text) {
    const element = documentObject.createElement(tagName);
    if (className) {
      element.className = className;
    }
    if (text !== undefined) {
      element.textContent = text;
    }
    return element;
  }

  function appendFact(documentObject, list, term, valueNodeOrText) {
    list.append(createElement(documentObject, "dt", "", term));
    const detail = createElement(documentObject, "dd", "", typeof valueNodeOrText === "string" ? valueNodeOrText : undefined);
    if (typeof valueNodeOrText !== "string") {
      detail.append(valueNodeOrText);
    }
    list.append(detail);
  }

  function safeCatalogLink(catalogUrl, route) {
    if (!route) {
      return null;
    }
    const resolved = new URL(route, catalogUrl);
    if (resolved.origin !== catalogUrl.origin) {
      return null;
    }
    return resolved.href;
  }

  function modelCard(documentObject, view, catalogUrl) {
    const item = createElement(documentObject, "li", "catalog-model");
    item.dataset.availability = view.availability;
    item.dataset.modelId = view.modelId;

    const header = createElement(documentObject, "div", "catalog-model-header");
    const identity = createElement(documentObject, "div", "catalog-model-identity");
    identity.append(
      createElement(documentObject, "h3", "", view.displayName),
      createElement(documentObject, "code", "catalog-model-id", view.modelId)
    );
    header.append(identity, createElement(documentObject, "span", `availability-badge ${view.availability}`, view.availabilityLabel));

    const description = createElement(documentObject, "p", "catalog-model-description", view.description);
    const facts = createElement(documentObject, "dl", "catalog-model-facts");
    let sourceValue = `${view.sourceKind}: ${view.sourceModel}`;
    if (view.sourceHuggingFaceModelId) {
      const sourceLink = createElement(documentObject, "a", "", sourceValue);
      sourceLink.href = `https://huggingface.co/${encodeURI(view.sourceHuggingFaceModelId)}`;
      sourceLink.target = "_blank";
      sourceLink.rel = "noopener noreferrer";
      sourceValue = sourceLink;
    }
    appendFact(documentObject, facts, "Source model", sourceValue);
    appendFact(documentObject, facts, "Quantization", view.quantization);
    appendFact(documentObject, facts, "Package", formatBytes(view.artifactByteCount));
    appendFact(documentObject, facts, "Availability", view.availabilityLabel);
    appendFact(documentObject, facts, "Quality boundary", view.qualityBoundary);

    const actions = createElement(documentObject, "div", "catalog-model-actions");
    const links = [
      ["Model card", view.modelCardRoute],
      ["Manifest", view.manifestRoute],
      ["Peer proof", view.peerOfferRoute]
    ];
    for (const [label, route] of links) {
      const href = safeCatalogLink(catalogUrl, route);
      if (!href) {
        continue;
      }
      const link = createElement(documentObject, "a", "", label);
      link.href = href;
      actions.append(link);
    }
    item.append(header, description, facts, actions);
    return item;
  }

  function renderCatalog(documentObject, views, catalogUrl) {
    const list = documentObject.getElementById("modelCatalogList");
    const queryInput = documentObject.getElementById("modelCatalogSearch");
    const filterSelect = documentObject.getElementById("modelCatalogAvailability");
    const count = documentObject.getElementById("modelCatalogVisibleCount");
    if (!list || !queryInput || !filterSelect) {
      return;
    }
    const renderFiltered = () => {
      const filtered = filterCatalogViews(views, queryInput.value, filterSelect.value);
      list.replaceChildren();
      for (const view of filtered) {
        list.append(modelCard(documentObject, view, catalogUrl));
      }
      if (!filtered.length) {
        list.append(createElement(documentObject, "li", "catalog-empty", "No catalog models match this view."));
      }
      if (count) {
        count.textContent = `${filtered.length} of ${views.length} models`;
      }
    };
    queryInput.addEventListener("input", renderFiltered);
    filterSelect.addEventListener("change", renderFiltered);
    renderFiltered();
  }

  async function loadCatalog(options = {}) {
    const documentObject = options.document || globalObject.document;
    const locationObject = options.location || globalObject.location;
    const fetchFunction = options.fetch || globalObject.fetch.bind(globalObject);
    const status = documentObject.getElementById("modelCatalogStatus");
    const catalogUrl = new URL("catalog/models.v0.json", locationObject.href);
    const catalogText = await fetchBoundedText(catalogUrl, fetchFunction);
    const catalog = validateCatalog(JSON.parse(catalogText), { siteOrigin: catalogUrl.origin });
    const announcements = [];
    const feedErrors = [];

    for (const route of (catalog.peer_source_announcement_routes || [])) {
      try {
        const feedUrl = new URL(route.route, catalogUrl);
        if (feedUrl.origin !== catalogUrl.origin) {
          throw new Error("announcement feed must be same-origin metadata");
        }
        const feedText = await fetchBoundedText(feedUrl, fetchFunction);
        const actualHash = await sha256Prefixed(feedText, options.crypto);
        if (actualHash !== route.sha256.toLowerCase()) {
          throw new Error("announcement feed SHA-256 did not match the catalog binding");
        }
        const feed = JSON.parse(feedText);
        announcements.push(...validateAnnouncementFeed(feed, catalog, { siteOrigin: catalogUrl.origin }));
      } catch (error) {
        feedErrors.push(error instanceof Error ? error.message : String(error));
      }
    }

    const views = buildCatalogView(catalog, announcements, { siteOrigin: catalogUrl.origin });
    renderCatalog(documentObject, views, catalogUrl);
    const activeModels = views.filter((view) => view.activePeerCount > 0).length;
    const activePeers = views.reduce((sum, view) => sum + view.activePeerCount, 0);
    const shareReadyModels = views.length - activeModels;
    const totalNode = documentObject.getElementById("modelCatalogTotal");
    const activeNode = documentObject.getElementById("modelCatalogActive");
    const readyNode = documentObject.getElementById("modelCatalogReady");
    if (totalNode) {
      totalNode.textContent = `${views.length} catalog ${views.length === 1 ? "model" : "models"}`;
    }
    if (activeNode) {
      activeNode.textContent = `${activeModels} active ${activeModels === 1 ? "model" : "models"} / ${activePeers} ${activePeers === 1 ? "peer" : "peers"}`;
    }
    if (readyNode) {
      readyNode.textContent = `${shareReadyModels} awaiting an active peer`;
    }
    if (status) {
      status.textContent = feedErrors.length
        ? `Catalog loaded. Peer availability failed closed: ${feedErrors.join("; ")}`
        : `Catalog and ${catalog.peer_source_announcement_routes.length} checksum-bound availability ${catalog.peer_source_announcement_routes.length === 1 ? "feed" : "feeds"} verified.`;
      status.dataset.state = feedErrors.length ? "warning" : "ready";
    }
    return { catalog, views, feedErrors };
  }

  const api = {
    MAX_METADATA_BYTES,
    buildCatalogView,
    fallbackDescription,
    filterCatalogViews,
    formatBytes,
    loadCatalog,
    sha256Prefixed,
    validateAnnouncementFeed,
    validateCatalog
  };

  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  }
  globalObject.MiniModelCatalog = api;

  if (globalObject.document && globalObject.location && typeof globalObject.fetch === "function") {
    const start = () => {
      loadCatalog().catch((error) => {
        const status = globalObject.document.getElementById("modelCatalogStatus");
        if (status) {
          status.textContent = `Catalog unavailable: ${error instanceof Error ? error.message : String(error)}`;
          status.dataset.state = "error";
        }
      });
    };
    if (globalObject.document.readyState === "loading") {
      globalObject.document.addEventListener("DOMContentLoaded", start, { once: true });
    } else {
      start();
    }
  }
})(typeof globalThis !== "undefined" ? globalThis : this);
