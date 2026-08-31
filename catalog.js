(function attachMiniModelCatalog(globalObject) {
  "use strict";

  const MAX_METADATA_BYTES = 1024 * 1024;
  const MAX_BROWSER_ADMISSION_BYTES = 64 * 1024;
  const CATALOG_SCHEMA = "minimodel.slm2_catalog.v1";
  const PEER_SCHEMA = "minimodel.slm2_peer_announcements.v1";
  const BROWSER_ADMISSION_SCHEMA = "tinyrustlm.browser-composition-admission/1";
  const DIRECT_COMPOSITION_PROTOCOL_PROFILE = "tinyrustlm.direct-composition-http.v1";
  const MEMBER_NAMES = Object.freeze([
    "model.slm2",
    "tokenizer.tokenizer2",
    "template.template2",
    "sampling.sampling2",
    "prompt.prompt2",
    "composition.acg2",
  ]);
  const DIRECT_PEER_DENIED_HOSTS = Object.freeze([
    "tinyrustlm.com",
    "minimodel.org",
    "slmcomposer.com",
    "lmcomposer.com",
    "mirust.com",
    "uaix.org",
    "memoryendpoints.com",
    "huggingface.co",
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
    if (value !== normalized
        || !/^\/[A-Za-z0-9._/-]+$/.test(normalized)
        || normalized.includes("//")
        || normalized.split("/").includes("..")) {
      throw new Error(`${label} must be a safe same-origin route`);
    }
    return normalized;
  }

  function strictIpv4Octets(hostname) {
    const parts = String(hostname || "").split(".");
    if (parts.length !== 4 || parts.some((part) => !/^(?:0|[1-9][0-9]{0,2})$/.test(part))) {
      return null;
    }
    const octets = parts.map(Number);
    return octets.some((octet) => octet > 255) ? null : octets;
  }

  function isPublicIpv4(octets) {
    const [first, second, third] = octets;
    return !(first === 0 || first === 10 || first === 127 || first >= 224
      || (first === 100 && second >= 64 && second <= 127)
      || (first === 169 && second === 254)
      || (first === 172 && second >= 16 && second <= 31)
      || (first === 192 && second === 0)
      || (first === 192 && second === 168)
      || (first === 198 && (second === 18 || second === 19))
      || (first === 198 && second === 51 && third === 100)
      || (first === 203 && second === 0 && third === 113));
  }

  function ipv6Words(hostname) {
    const host = String(hostname || "").replace(/^\[|\]$/g, "").toLowerCase();
    if (!host || host.includes("%") || host.includes(".")) {
      return null;
    }
    const halves = host.split("::");
    if (halves.length > 2) {
      return null;
    }
    const parseHalf = (half) => half
      ? half.split(":").map((part) => /^[0-9a-f]{1,4}$/.test(part) ? Number.parseInt(part, 16) : NaN)
      : [];
    const left = parseHalf(halves[0]);
    const right = halves.length === 2 ? parseHalf(halves[1]) : [];
    if (left.some(Number.isNaN) || right.some(Number.isNaN)) {
      return null;
    }
    if (halves.length === 1) {
      return left.length === 8 ? left : null;
    }
    const omitted = 8 - left.length - right.length;
    return omitted > 0 ? [...left, ...new Array(omitted).fill(0), ...right] : null;
  }

  function isPublicIpv6(words) {
    if (!words || (words[0] & 0xe000) !== 0x2000) {
      return false;
    }
    if (words.slice(0, 5).every((word) => word === 0) && words[5] === 0xffff) {
      return false;
    }
    if (words[0] === 0x2002 || words[0] === 0x3ffe) {
      return false;
    }
    if (words[0] !== 0x2001) {
      return true;
    }
    if (words[1] === 0 || words[1] === 0x0db8) {
      return false;
    }
    if (words[1] === 2 && words[2] === 0) {
      return false;
    }
    return (words[1] & 0xfff0) !== 0x0010 && (words[1] & 0xfff0) !== 0x0020;
  }

  function isDeniedDirectPeerHost(hostname) {
    const host = String(hostname || "").toLowerCase();
    return DIRECT_PEER_DENIED_HOSTS.some((denied) => host === denied || host.endsWith(`.${denied}`));
  }

  function requirePublicDnsName(hostname) {
    if (hostname.length > 253 || !hostname.includes(".")
        || !/^[a-z0-9.-]+$/.test(hostname) || hostname.includes("..")
        || hostname.split(".").some((label) => !label || label.length > 63
          || label.startsWith("-") || label.endsWith("-"))) {
      throw new Error("peer announcement.peer_base_url host must be public DNS or a supported IP literal");
    }
    if (/(?:^|\.)(?:example|internal|invalid|lan|local|localhost)$/.test(hostname)
        || hostname.endsWith(".home.arpa")
        || ["example.com", "example.net", "example.org"].some((reserved) =>
          hostname === reserved || hostname.endsWith(`.${reserved}`))) {
      throw new Error("peer announcement.peer_base_url host must not use a local or reserved DNS name");
    }
  }

  function splitDirectPeerAuthority(authority) {
    if (!authority || /[%@\\\s\u0000-\u001f\u007f]/.test(authority)) {
      throw new Error("peer announcement.peer_base_url authority contains an encoded or unsafe alias");
    }
    let host;
    let portText = "";
    if (authority.startsWith("[")) {
      const closing = authority.indexOf("]");
      if (closing < 0) {
        throw new Error("peer announcement.peer_base_url IPv6 host must be bracketed");
      }
      host = authority.slice(0, closing + 1);
      const suffix = authority.slice(closing + 1);
      if (suffix) {
        if (!/^:[0-9]{1,5}$/.test(suffix)) {
          throw new Error("peer announcement.peer_base_url port is invalid");
        }
        portText = suffix.slice(1);
      }
    } else {
      if ((authority.match(/:/g) || []).length > 1) {
        throw new Error("peer announcement.peer_base_url IPv6 host must be bracketed");
      }
      const separator = authority.lastIndexOf(":");
      if (separator >= 0) {
        host = authority.slice(0, separator);
        portText = authority.slice(separator + 1);
        if (!/^[0-9]{1,5}$/.test(portText)) {
          throw new Error("peer announcement.peer_base_url port is invalid");
        }
      } else {
        host = authority;
      }
    }
    if (!host || host.endsWith(".")) {
      throw new Error("peer announcement.peer_base_url host must not be empty or end in a dot");
    }
    if (portText) {
      const port = Number(portText);
      if (!Number.isInteger(port) || port < 1 || port > 65535 || String(port) !== portText) {
        throw new Error("peer announcement.peer_base_url port must be an integer from 1 through 65535");
      }
    }
    return { host, portText };
  }

  function normalizeDirectPeerBaseUrl(value) {
    const raw = requireString(value, "peer announcement.peer_base_url", 512);
    if (raw !== value || /[\u0000-\u001f\u007f]/.test(raw)) {
      throw new Error("peer announcement.peer_base_url must be a bounded printable string");
    }
    const match = /^(https?):\/\/([^/?#]+)(\/?)$/i.exec(raw);
    if (!match) {
      throw new Error("peer announcement.peer_base_url must be an absolute root HTTP(S) origin");
    }
    const rawProtocol = `${match[1].toLowerCase()}:`;
    const authority = splitDirectPeerAuthority(match[2]);
    let parsed;
    try {
      parsed = new URL(raw);
    } catch (_error) {
      throw new Error("peer announcement.peer_base_url must be an absolute root HTTP(S) origin");
    }
    if (parsed.protocol !== rawProtocol || parsed.username || parsed.password
        || parsed.search || parsed.hash || parsed.pathname !== "/") {
      throw new Error("peer announcement.peer_base_url contains credentials or an unexpected endpoint path");
    }

    const rawHost = authority.host.toLowerCase();
    const parsedHost = parsed.hostname.toLowerCase();
    const bareHost = parsedHost.replace(/^\[|\]$/g, "");
    const loopback = bareHost === "localhost" || bareHost === "127.0.0.1" || bareHost === "::1";
    if (rawProtocol === "http:" && (!loopback || !authority.portText)) {
      throw new Error("peer announcement.peer_base_url HTTP requires an exact loopback host and bounded port");
    }
    if (loopback) {
      const exactLoopback = bareHost === "::1" ? "[::1]" : bareHost;
      if (rawHost !== exactLoopback) {
        throw new Error("peer announcement.peer_base_url loopback host must use its exact canonical spelling");
      }
    } else if (parsedHost.startsWith("[")) {
      if (!isPublicIpv6(ipv6Words(parsedHost))) {
        throw new Error("peer announcement.peer_base_url IPv6 address must be public and non-reserved");
      }
    } else {
      const ipv4 = strictIpv4Octets(parsedHost);
      if (ipv4) {
        if (rawHost !== parsedHost) {
          throw new Error("peer announcement.peer_base_url IPv4 host must not use a numeric alias");
        }
        if (!isPublicIpv4(ipv4)) {
          throw new Error("peer announcement.peer_base_url IPv4 address must be public and non-reserved");
        }
      } else {
        if (rawHost !== parsedHost) {
          throw new Error("peer announcement.peer_base_url DNS host must not use an encoded or normalized alias");
        }
        requirePublicDnsName(parsedHost);
      }
    }
    if (isDeniedDirectPeerHost(bareHost)) {
      throw new Error("project and service hosts cannot serve direct peer model bytes");
    }
    if (rawProtocol === "http:") {
      return `http://${parsedHost}:${authority.portText}/`;
    }
    return `${parsed.origin}/`;
  }

  function validateBrowserAdmission(value, label) {
    requireExactKeys(value, ["route", "bytes", "sha256"], label);
    requireLocalRoute(value.route, `${label}.route`);
    if (!Number.isSafeInteger(value.bytes)
        || value.bytes < 1
        || value.bytes > MAX_BROWSER_ADMISSION_BYTES) {
      throw new Error(`${label}.bytes must be a positive bounded safe integer`);
    }
    const digest = requireString(value.sha256, `${label}.sha256`, 71);
    if (digest !== value.sha256 || !/^sha256:[a-f0-9]{64}$/.test(digest)) {
      throw new Error(`${label}.sha256 must be a lower-case prefixed SHA-256 identity`);
    }
    return value;
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
      "browser_admission",
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

    validateBrowserAdmission(value.browser_admission, `${label}.browser_admission`);

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
      "browser_admission_schema",
      "entries",
    ], "catalog");
    if (value.schema !== CATALOG_SCHEMA
        || value.qualification_policy !== "exact-current-evidence-required") {
      throw new Error("catalog schema or qualification policy is unsupported");
    }
    if (value.browser_admission_schema !== BROWSER_ADMISSION_SCHEMA) {
      throw new Error("catalog browser admission schema is unsupported");
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
    if (value.protocol_profile !== DIRECT_COMPOSITION_PROTOCOL_PROFILE) {
      throw new Error("peer announcement protocol profile is unsupported");
    }
    if (value.consent_status !== "explicit"
        || value.outside_proof_status !== "current-passed") {
      throw new Error("peer announcement lacks explicit consent or current outside proof");
    }
    const peerUrl = normalizeDirectPeerBaseUrl(value.peer_base_url);
    const expires = requireUtc(value.expires_utc, "peer announcement.expires_utc");
    if (Date.parse(expires) <= now) {
      throw new Error("peer announcement is expired");
    }
    return { id, compositionId, peerUrl, expires };
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
        browserAdmission: Object.freeze({
          schema: BROWSER_ADMISSION_SCHEMA,
          route: entry.browser_admission.route,
          bytes: entry.browser_admission.bytes,
          sha256: entry.browser_admission.sha256,
        }),
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
    BROWSER_ADMISSION_SCHEMA,
    DIRECT_COMPOSITION_PROTOCOL_PROFILE,
    MEMBER_NAMES,
    MAX_METADATA_BYTES,
    MAX_BROWSER_ADMISSION_BYTES,
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
