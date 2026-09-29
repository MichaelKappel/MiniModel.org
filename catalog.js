(function attachMiniModelCatalog(globalObject) {
  "use strict";

  const MAX_METADATA_BYTES = 1024 * 1024;
  const MAX_METADATA_CHUNKS = 4096;
  const DEFAULT_LOAD_TIMEOUT_MILLISECONDS = 10000;
  const MAX_LOAD_TIMEOUT_MILLISECONDS = 60000;
  const MAX_BROWSER_ADMISSION_BYTES = 64 * 1024;
  const MAX_QUALIFICATION_RECEIPT_BYTES = 128 * 1024;
  const MAX_SOURCE_AGE_MILLISECONDS = 24 * 60 * 60 * 1000;
  const MAX_FUTURE_SKEW_MILLISECONDS = 5 * 60 * 1000;
  const MAX_SOURCE_LIFETIME_MILLISECONDS = 24 * 60 * 60 * 1000;
  const MAX_ENTRIES = 128;
  const MAX_ANNOUNCEMENTS = 512;
  const MAX_VERIFICATION_KEYS = 32;
  const MAX_REVOCATIONS = 1024;
  const MAX_SUPERSESSIONS = 1024;
  const CATALOG_SCHEMA = "minimodel.slm2_catalog.v1";
  const PEER_SCHEMA = "minimodel.slm2_peer_announcements.v1";
  const BROWSER_ADMISSION_SCHEMA = "tinyrustlm.browser-composition-admission/1";
  const QUALIFICATION_RECEIPT_SCHEMA = "tinyrustlm.qualification-receipt/1";
  const PEER_PROOF_SCHEMA = "tinyrustlm.peer-announcement-proof/1";
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
    if (typeof value !== "string"
        || value.length < 1
        || value.length > maximum
        || value !== value.trim()
        || /[\u0000-\u001f\u007f\ud800-\udfff]/u.test(value)) {
      throw new Error(`${label} must be a bounded printable string`);
    }
    return value;
  }

  function requireId(value, label) {
    const normalized = requireString(value, label, 128);
    if (!/^[a-z0-9][a-z0-9._/-]*$/.test(normalized)) {
      throw new Error(`${label} must be lower-case route-safe ASCII`);
    }
    return normalized;
  }

  function requireSha256(value, label) {
    const normalized = requireString(value, label, 71);
    if (!/^sha256:[a-f0-9]{64}$/u.test(normalized)) {
      throw new Error(`${label} must be a lower-case prefixed SHA-256 identity`);
    }
    return normalized;
  }

  function requireUtc(value, label) {
    const normalized = requireString(value, label, 20);
    const milliseconds = Date.parse(normalized);
    const canonical = Number.isFinite(milliseconds)
      ? new Date(milliseconds).toISOString().replace(".000Z", "Z")
      : "";
    if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/u.test(normalized)
        || !Number.isFinite(milliseconds)
        || canonical !== normalized) {
      throw new Error(`${label} must be a UTC timestamp with second precision`);
    }
    return normalized;
  }

  function requireRevision(value, label) {
    if (!Number.isSafeInteger(value) || value < 1 || value > Number.MAX_SAFE_INTEGER) {
      throw new Error(`${label} must be a positive safe integer`);
    }
    return value;
  }

  function validationNow(options = {}) {
    const now = options.now instanceof Date ? options.now.getTime() : Date.now();
    if (!Number.isFinite(now)) {
      throw new Error("catalog validation time is invalid");
    }
    return now;
  }

  function requireFreshWindow(generatedAtUtc, validUntilUtc, now, label) {
    const generated = Date.parse(requireUtc(generatedAtUtc, `${label}.generated_at_utc`));
    const validUntil = Date.parse(requireUtc(validUntilUtc, `${label}.valid_until_utc`));
    if (generated > now + MAX_FUTURE_SKEW_MILLISECONDS
        || generated < now - MAX_SOURCE_AGE_MILLISECONDS
        || validUntil <= now
        || validUntil <= generated
        || validUntil - generated > MAX_SOURCE_LIFETIME_MILLISECONDS) {
      throw new Error(`${label} is stale, future-dated, or has an excessive lifetime`);
    }
    return { generated, validUntil };
  }

  function requireBase64Url(value, byteLength, label) {
    const expectedLength = Math.ceil(byteLength * 4 / 3);
    if (typeof value !== "string"
        || value.length !== expectedLength
        || !/^[A-Za-z0-9_-]+$/u.test(value)) {
      throw new Error(`${label} must be canonical unpadded base64url`);
    }
    const padding = "=".repeat((4 - (value.length % 4)) % 4);
    let binary;
    try {
      binary = globalObject.atob(
        value.replaceAll("-", "+").replaceAll("_", "/") + padding,
      );
    } catch (_error) {
      throw new Error(`${label} must be canonical unpadded base64url`);
    }
    if (binary.length !== byteLength) {
      throw new Error(`${label} must have the required decoded length`);
    }
    let bytes = "";
    for (let index = 0; index < binary.length; index += 1) {
      bytes += String.fromCharCode(binary.charCodeAt(index));
    }
    const canonical = globalObject.btoa(bytes)
      .replaceAll("+", "-")
      .replaceAll("/", "_")
      .replace(/=+$/u, "");
    if (canonical !== value) {
      throw new Error(`${label} must be canonical unpadded base64url`);
    }
    return value;
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

  function normalizeDirectPeerBaseUrl(value, options = {}) {
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
    // Only the operator-owned HTTPS bootstrap seed is exempt from the IP rule.
    const bootstrapSeed = rawHost === "minimodel.org"
      && parsed.origin === "https://minimodel.org";
    const exactLoopback = (rawHost === "127.0.0.1" && parsedHost === "127.0.0.1")
      || (rawHost === "[::1]" && parsedHost === "[::1]");
    if (rawProtocol !== "https:" && !exactLoopback) {
      throw new Error("peer announcement.peer_base_url HTTP requires an exact loopback IP literal");
    }
    if (!bootstrapSeed && isDeniedDirectPeerHost(bareHost)) {
      throw new Error("project and service hosts cannot serve direct peer model bytes");
    }
    if (!bootstrapSeed && !exactLoopback && parsedHost.startsWith("[")) {
      if (!isPublicIpv6(ipv6Words(parsedHost))) {
        throw new Error("peer announcement.peer_base_url IPv6 address must be public and non-reserved");
      }
    } else if (!bootstrapSeed && !exactLoopback) {
      const ipv4 = strictIpv4Octets(parsedHost);
      if (ipv4) {
        if (rawHost !== parsedHost) {
          throw new Error("peer announcement.peer_base_url IPv4 host must not use a numeric alias");
        }
        if (!isPublicIpv4(ipv4)) {
          throw new Error("peer announcement.peer_base_url IPv4 address must be public and non-reserved");
        }
      } else {
        requirePublicDnsName(parsedHost);
        throw new Error("browser peer routes cannot pin DNS resolution and require a public or loopback IP literal");
      }
    }
    const applicationOrigin = options.applicationOrigin === undefined
      ? globalObject.location?.origin : options.applicationOrigin;
    if (applicationOrigin !== undefined && applicationOrigin !== null) {
      let application;
      try { application = new URL(applicationOrigin); } catch (_error) {
        throw new Error("application origin was invalid");
      }
      if (!/^https?:$/u.test(application.protocol) || application.origin === "null"
          || (!bootstrapSeed && application.origin === parsed.origin)) {
        throw new Error("the application origin cannot serve direct peer bytes");
      }
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
    if (typeof value.project_server_serves_model_bytes !== "boolean"
        || value.project_server_proxies_model_bytes !== false
        || (includeInference && value.project_server_runs_inference !== false)) {
      throw new Error(`${label} must declare seed hosting and keep proxying and inference off project servers`);
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
    const wireNames = value.composition.members.map((member) => member.name);
    const memberNames = value.composition.members.map((member) => member.name).sort();
    if (memberNames.some((name, index) => name !== [...MEMBER_NAMES].sort()[index])) {
      throw new Error(`${label}.composition member set is not closed`);
    }
    if (wireNames.some((name, index) => name !== MEMBER_NAMES[index])) {
      throw new Error(`${label}.composition members must use canonical P2P six-member wire order`);
    }

    validateBrowserAdmission(value.browser_admission, `${label}.browser_admission`);

    requireExactKeys(value.profiles, ["conversion", "quantization", "runtime"], `${label}.profiles`);
    requireId(value.profiles.conversion, `${label}.profiles.conversion`);
    requireId(value.profiles.quantization, `${label}.profiles.quantization`);
    requireId(value.profiles.runtime, `${label}.profiles.runtime`);

    requireExactKeys(value.qualification, [
      "status",
      "receipt_id",
      "receipt_route",
      "receipt_bytes",
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
    requireSha256(value.qualification.receipt_id, `${label}.qualification.receipt_id`);
    requireLocalRoute(value.qualification.receipt_route, `${label}.qualification.receipt_route`);
    if (!Number.isSafeInteger(value.qualification.receipt_bytes)
        || value.qualification.receipt_bytes < 2
        || value.qualification.receipt_bytes > MAX_QUALIFICATION_RECEIPT_BYTES) {
      throw new Error(`${label}.qualification.receipt_bytes is outside its bound`);
    }
    requireSha256(value.qualification.receipt_sha256, `${label}.qualification.receipt_sha256`);

    if (!Array.isArray(value.peer_announcement_ids)
        || value.peer_announcement_ids.length > MAX_ANNOUNCEMENTS
        || new Set(value.peer_announcement_ids).size !== value.peer_announcement_ids.length) {
      throw new Error(`${label}.peer_announcement_ids must be a bounded unique array`);
    }
    value.peer_announcement_ids.forEach((item, index) => requireId(item, `${label}.peer_announcement_ids[${index}]`));
    return value;
  }

  function validatePeerVerificationKey(value, label, now) {
    requireExactKeys(value, [
      "key_id",
      "algorithm",
      "public_key_base64url",
      "not_before_utc",
      "not_after_utc",
    ], label);
    const keyId = requireSha256(value.key_id, `${label}.key_id`);
    if (value.algorithm !== "Ed25519") {
      throw new Error(`${label}.algorithm is unsupported`);
    }
    const notBeforeUtc = requireUtc(value.not_before_utc, `${label}.not_before_utc`);
    const notAfterUtc = requireUtc(value.not_after_utc, `${label}.not_after_utc`);
    if (Date.parse(notBeforeUtc) > now + MAX_FUTURE_SKEW_MILLISECONDS
        || Date.parse(notAfterUtc) <= now
        || Date.parse(notAfterUtc) <= Date.parse(notBeforeUtc)) {
      throw new Error(`${label} is not currently valid`);
    }
    requireBase64Url(value.public_key_base64url, 32, `${label}.public_key_base64url`);
    return {
      keyId,
      publicKey: value.public_key_base64url,
      notBeforeUtc,
      notAfterUtc,
    };
  }

  function validateSupersession(value, label) {
    requireExactKeys(value, [
      "superseded_composition_id",
      "replacement_composition_id",
      "effective_at_utc",
    ], label);
    const superseded = requireSha256(
      value.superseded_composition_id,
      `${label}.superseded_composition_id`,
    );
    const replacement = requireSha256(
      value.replacement_composition_id,
      `${label}.replacement_composition_id`,
    );
    if (superseded === replacement) {
      throw new Error(`${label} must replace a different composition`);
    }
    requireUtc(value.effective_at_utc, `${label}.effective_at_utc`);
    return value;
  }

  function validateCatalog(value, options = {}) {
    requireExactKeys(value, [
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
    ], "catalog");
    if (value.schema !== CATALOG_SCHEMA
        || value.qualification_policy !== "exact-current-evidence-required") {
      throw new Error("catalog schema or qualification policy is unsupported");
    }
    if (value.browser_admission_schema !== BROWSER_ADMISSION_SCHEMA) {
      throw new Error("catalog browser admission schema is unsupported");
    }
    if (value.qualification_receipt_schema !== QUALIFICATION_RECEIPT_SCHEMA) {
      throw new Error("catalog qualification receipt schema is unsupported");
    }
    const now = validationNow(options);
    requireId(value.catalog_id, "catalog.catalog_id");
    requireRevision(value.revision, "catalog.revision");
    requireFreshWindow(value.generated_at_utc, value.valid_until_utc, now, "catalog");
    requireProjectBoundary(value, true, "catalog");
    if (!Array.isArray(value.peer_verification_keys)
        || value.peer_verification_keys.length > MAX_VERIFICATION_KEYS) {
      throw new Error("catalog.peer_verification_keys must be a bounded array");
    }
    const peerKeys = value.peer_verification_keys.map((key, index) =>
      validatePeerVerificationKey(key, `catalog.peer_verification_keys[${index}]`, now));
    if (new Set(peerKeys.map((key) => key.keyId)).size !== peerKeys.length) {
      throw new Error("catalog contains a duplicate peer verification key");
    }
    if (!Array.isArray(value.revoked_qualification_receipt_ids)
        || value.revoked_qualification_receipt_ids.length > MAX_REVOCATIONS) {
      throw new Error("catalog revocation set must be a bounded array");
    }
    value.revoked_qualification_receipt_ids.forEach((receiptId, index) => {
      requireSha256(receiptId, `catalog.revoked_qualification_receipt_ids[${index}]`);
    });
    if (new Set(value.revoked_qualification_receipt_ids).size
        !== value.revoked_qualification_receipt_ids.length) {
      throw new Error("catalog contains a duplicate qualification revocation");
    }
    if (!Array.isArray(value.supersessions)
        || value.supersessions.length > MAX_SUPERSESSIONS) {
      throw new Error("catalog supersession set must be a bounded array");
    }
    value.supersessions.forEach((edge, index) => {
      validateSupersession(edge, `catalog.supersessions[${index}]`);
    });
    if (new Set(value.supersessions.map((edge) => edge.superseded_composition_id)).size
        !== value.supersessions.length) {
      throw new Error("catalog supersedes one composition more than once");
    }
    if (!Array.isArray(value.entries) || value.entries.length > MAX_ENTRIES) {
      throw new Error("catalog.entries must be a bounded array");
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
      if (value.revoked_qualification_receipt_ids.includes(entry.qualification.receipt_id)
          || value.supersessions.some((edge) =>
            edge.superseded_composition_id === entry.composition.composition_id
              && Date.parse(edge.effective_at_utc) <= Date.parse(value.generated_at_utc))) {
        throw new Error("catalog publishes a revoked or superseded entry as current");
      }
    });
    return value;
  }

  function validatePeerAnnouncement(value, catalogByComposition, catalogKeys, now, feedWindow, options) {
    requireExactKeys(value, [
      "announcement_id",
      "composition_id",
      "piece_set_sha256",
      "peer_base_url",
      "protocol_profile",
      "consent_status",
      "consent_receipt_sha256",
      "outside_proof_status",
      "outside_proof_receipt_sha256",
      "issued_at_utc",
      "expires_utc",
      "verification_key_id",
      "signature",
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
    if (value.consent_status !== "explicit") {
      throw new Error("peer announcement lacks explicit consent");
    }
    requireSha256(value.consent_receipt_sha256, "peer announcement.consent_receipt_sha256");
    const peerUrl = normalizeDirectPeerBaseUrl(value.peer_base_url, options);
    // Operator renewal asserts hosting responsibility, never an independent observation.
    if (peerUrl === "https://minimodel.org/") {
      if (value.outside_proof_status !== "not-required-operator-seed"
          || value.outside_proof_receipt_sha256 !== null) {
        throw new Error("operator seed requires its explicit disposition and a null outside-proof receipt");
      }
    } else {
      if (value.outside_proof_status !== "current-passed") {
        throw new Error("third-party peer requires current outside proof");
      }
      requireSha256(value.outside_proof_receipt_sha256, "peer announcement.outside_proof_receipt_sha256");
    }
    const issued = requireUtc(value.issued_at_utc, "peer announcement.issued_at_utc");
    const expires = requireUtc(value.expires_utc, "peer announcement.expires_utc");
    if (Date.parse(issued) > now + MAX_FUTURE_SKEW_MILLISECONDS
        || Date.parse(issued) < now - MAX_SOURCE_AGE_MILLISECONDS
        || Date.parse(expires) <= now
        || Date.parse(expires) <= Date.parse(issued)
        || Date.parse(expires) - Date.parse(issued) > MAX_SOURCE_LIFETIME_MILLISECONDS) {
      throw new Error("peer announcement is stale, future-dated, or expired");
    }
    if (Date.parse(issued) > feedWindow.generated
        || Date.parse(expires) > feedWindow.validUntil) {
      throw new Error("peer announcement lies outside its feed validity window");
    }
    const verificationKeyId = requireSha256(
      value.verification_key_id,
      "peer announcement.verification_key_id",
    );
    requireBase64Url(value.signature, 64, "peer announcement.signature");
    const verificationKey = catalogKeys.get(verificationKeyId);
    if (!verificationKey
        || Date.parse(verificationKey.notBeforeUtc) > Date.parse(issued)
        || Date.parse(verificationKey.notAfterUtc) < Date.parse(expires)) {
      throw new Error("peer announcement is not covered by a current catalog verifier");
    }
    return { id, compositionId, peerUrl, issued, expires, verificationKeyId };
  }

  function validatePeerFeed(value, catalogValue, options = {}) {
    const catalog = validateCatalog(catalogValue, options);
    requireExactKeys(value, [
      "schema",
      "feed_id",
      "revision",
      "generated_at_utc",
      "valid_until_utc",
      "project_server_serves_model_bytes",
      "project_server_proxies_model_bytes",
      "announcements",
    ], "peer feed");
    if (value.schema !== PEER_SCHEMA) {
      throw new Error("peer feed schema is unsupported");
    }
    requireId(value.feed_id, "peer feed.feed_id");
    requireRevision(value.revision, "peer feed.revision");
    requireProjectBoundary(value, false, "peer feed");
    if (!Array.isArray(value.announcements)
        || value.announcements.length > MAX_ANNOUNCEMENTS) {
      throw new Error("peer feed.announcements must be a bounded array");
    }
    const now = validationNow(options);
    const feedWindow = requireFreshWindow(
      value.generated_at_utc,
      value.valid_until_utc,
      now,
      "peer feed",
    );
    if (Date.parse(value.generated_at_utc) < Date.parse(catalog.generated_at_utc)
        || Date.parse(value.valid_until_utc) > Date.parse(catalog.valid_until_utc)) {
      throw new Error("peer feed is not bounded by the current catalog window");
    }
    const catalogByComposition = new Map(catalog.entries.map((entry) => [
      entry.composition.composition_id,
      entry,
    ]));
    const catalogKeys = new Map(catalog.peer_verification_keys.map((key) => [
      key.key_id,
      validatePeerVerificationKey(key, "catalog peer verification key", now),
    ]));
    const catalogByPeerId = new Map();
    catalog.entries.forEach((entry) => {
      entry.peer_announcement_ids.forEach((peerId) => {
        if (catalogByPeerId.has(peerId)) {
          throw new Error("catalog declares one peer announcement more than once");
        }
        catalogByPeerId.set(peerId, entry.composition.composition_id);
      });
    });
    const ids = new Set();
    const announcements = value.announcements.map((announcement) => {
      const validated = validatePeerAnnouncement(
        announcement,
        catalogByComposition,
        catalogKeys,
        now,
        feedWindow,
        options,
      );
      if (ids.has(validated.id)) {
        throw new Error("peer feed contains a duplicate announcement");
      }
      if (catalogByPeerId.get(validated.id) !== validated.compositionId) {
        throw new Error("peer announcement is not declared by its catalog entry");
      }
      ids.add(validated.id);
      return validated;
    });
    if (ids.size !== catalogByPeerId.size
        || [...catalogByPeerId.keys()].some((id) => !ids.has(id))) {
      throw new Error("catalog and peer feed announcement identities are not closed");
    }
    return announcements;
  }

  function buildCatalogView(catalogValue, peerFeedValue, options = {}) {
    const catalog = validateCatalog(catalogValue, options);
    const announcements = validatePeerFeed(peerFeedValue, catalog, options);
    if (announcements.length !== 0) {
      throw new Error("peer-backed views require asynchronous Ed25519 verification");
    }
    return projectCatalog(catalog, announcements);
  }

  function peerAnnouncementSigningBytes(announcement) {
    return new TextEncoder().encode(`${JSON.stringify({
      schema: PEER_PROOF_SCHEMA,
      announcement_id: announcement.announcement_id,
      composition_id: announcement.composition_id,
      piece_set_sha256: announcement.piece_set_sha256,
      peer_base_url: announcement.peer_base_url,
      protocol_profile: announcement.protocol_profile,
      consent_status: announcement.consent_status,
      consent_receipt_sha256: announcement.consent_receipt_sha256,
      outside_proof_status: announcement.outside_proof_status,
      outside_proof_receipt_sha256: announcement.outside_proof_receipt_sha256,
      issued_at_utc: announcement.issued_at_utc,
      expires_utc: announcement.expires_utc,
      verification_key_id: announcement.verification_key_id,
    })}\n`);
  }

  function decodeBase64Url(value, byteLength, label) {
    requireBase64Url(value, byteLength, label);
    const padding = "=".repeat((4 - (value.length % 4)) % 4);
    const binary = globalObject.atob(value.replaceAll("-", "+").replaceAll("_", "/") + padding);
    return Uint8Array.from(binary, (character) => character.charCodeAt(0));
  }

  async function buildVerifiedCatalogView(catalogValue, peerFeedValue, options = {}) {
    throwIfAborted(options.signal);
    // Own the validated inputs across crypto awaits; caller mutations cannot change the view.
    const catalog = parseCanonicalJsonText(`${JSON.stringify(catalogValue)}\n`);
    const peerFeed = parseCanonicalJsonText(`${JSON.stringify(peerFeedValue)}\n`);
    const announcements = validatePeerFeed(peerFeed, catalog, options);
    if (announcements.length) {
      const subtle = (options.crypto || globalObject.crypto)?.subtle;
      if (!subtle || typeof subtle.importKey !== "function" || typeof subtle.verify !== "function") {
        throw new Error("Ed25519 verification is unavailable");
      }
      for (const peer of announcements) {
        const announcement = peerFeed.announcements.find((item) => item.announcement_id === peer.id);
        const verifier = catalog.peer_verification_keys.find((item) => item.key_id === peer.verificationKeyId);
        const publicKey = decodeBase64Url(verifier.public_key_base64url, 32, "peer public key");
        const signature = decodeBase64Url(announcement.signature, 64, "peer signature");
        // The browser signs the normalized root origin, including its trailing slash.
        const bytes = peerAnnouncementSigningBytes({ ...announcement, peer_base_url: peer.peerUrl });
        let valid;
        try {
          const key = await waitForCatalog(subtle.importKey("raw", publicKey, { name: "Ed25519" }, false, ["verify"]), options.signal);
          throwIfAborted(options.signal);
          valid = await waitForCatalog(subtle.verify({ name: "Ed25519" }, key, signature, bytes), options.signal);
        } catch (_error) {
          throwIfAborted(options.signal);
          throw new Error("peer announcement signature verification failed");
        }
        if (valid !== true) throw new Error("peer announcement signature is invalid");
      }
    }
    throwIfAborted(options.signal);
    return projectCatalog(catalog, announcements);
  }

  function projectCatalog(catalog, announcements) {
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
        qualification: Object.freeze({
          schema: QUALIFICATION_RECEIPT_SCHEMA,
          receiptId: entry.qualification.receipt_id,
          receiptRoute: entry.qualification.receipt_route,
          receiptBytes: entry.qualification.receipt_bytes,
          receiptSha256: entry.qualification.receipt_sha256,
        }),
        quantizationProfile: entry.profiles.quantization,
        licenseRoute: entry.license.route,
        availability: peers.length ? "peer" : "local-conversion",
        availabilityLabel: peers.length
          ? `${peers.length} signed peer ${peers.length === 1 ? "announcement" : "announcements"}; reachability not proven`
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
    queryInput.disabled = false;
    filterSelect.disabled = false;
    queryInput.oninput = renderFiltered;
    filterSelect.onchange = renderFiltered;
    renderFiltered();
  }

  function parseCanonicalJsonText(text) {
    if (typeof text !== "string") {
      throw new Error("metadata must be canonical JSON text");
    }
    const byteLength = new TextEncoder().encode(text).length;
    if (byteLength < 2 || byteLength > MAX_METADATA_BYTES || !text.endsWith("\n")) {
      throw new Error("metadata was not bounded canonical JSON");
    }
    let parsed;
    try {
      parsed = JSON.parse(text);
    } catch (_error) {
      throw new Error("metadata was not valid JSON");
    }
    if (`${JSON.stringify(parsed)}\n` !== text) {
      throw new Error("metadata JSON bytes were not canonical");
    }
    // JSON.parse admits escaped lone surrogates; every key and value must be scalar text.
    // Iterate the already byte-bounded tree without recursive call-stack growth.
    const pending = [parsed];
    while (pending.length) {
      const value = pending.pop();
      if (typeof value === "string" && /[\ud800-\udfff]/u.test(value)) {
        throw new Error("metadata JSON contained an unpaired surrogate");
      }
      if (value && typeof value === "object") {
        for (const [key, child] of Object.entries(value)) {
          if (/[\ud800-\udfff]/u.test(key)) throw new Error("metadata JSON contained an unpaired surrogate");
          pending.push(child);
        }
      }
    }
    return parsed;
  }

  // Preserve the shared deadline, explicit cancellation or supersession reason.
  function throwIfAborted(signal) {
    if (signal?.aborted) throw signal.reason || new Error("Catalog load was cancelled");
  }

  // Observe even late rejection, while letting the caller stop waiting for an uncooperative dependency.
  function waitForCatalog(task, signal) {
    if (!signal) return Promise.resolve(task);
    return new Promise((resolve, reject) => {
      const aborted = () => reject(signal.reason || new Error("Catalog load was cancelled"));
      signal.addEventListener("abort", aborted, { once: true });
      Promise.resolve(task).then(value => {
        signal.removeEventListener("abort", aborted);
        if (signal.aborted) aborted(); else resolve(value);
      }, error => {
        signal.removeEventListener("abort", aborted);
        reject(error);
      });
      if (signal.aborted) aborted();
    });
  }

  // Cancellation is best effort for injected streams; never wait indefinitely for their cleanup.
  function cancelCatalogBody(body) {
    try { Promise.resolve(body?.cancel()).catch(() => {}); } catch (_) { /* Already closed/locked. */ }
  }

  // Bound decoded bytes and chunk count before concatenation; require complete fatal UTF-8 decoding.
  async function fetchJson(url, fetchImplementation, signal) {
    throwIfAborted(signal);
    const request = Promise.resolve().then(() => {
      throwIfAborted(signal);
      return fetchImplementation(url, { method: "GET", cache: "no-store", credentials: "omit", redirect: "error", signal });
    });
    // A late Response belongs to the cancelled operation and must not retain an unread body.
    request.then(response => { if (signal.aborted) cancelCatalogBody(response?.body); }, () => {});
    const response = await waitForCatalog(request, signal);
    let reader;
    let complete = false;
    try {
      throwIfAborted(signal);
      if (!response.ok) throw new Error(`metadata request failed with HTTP ${response.status}`);
      const declared = response.headers?.get("content-length");
      if (declared !== null && declared !== undefined &&
          (!/^(0|[1-9][0-9]*)$/.test(declared) || Number(declared) > MAX_METADATA_BYTES)) {
        throw new Error("metadata response exceeded the byte limit");
      }
      if (!response.body || typeof response.body.getReader !== "function") throw new Error("metadata response stream is unavailable");
      reader = response.body.getReader();
      const decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
      let text = "";
      let bytes = 0;
      let chunks = 0;
      while (true) {
        const { done, value } = await waitForCatalog(reader.read(), signal);
        throwIfAborted(signal);
        if (done) break;
        if (!(value instanceof Uint8Array) || ++chunks > MAX_METADATA_CHUNKS || value.byteLength > MAX_METADATA_BYTES - bytes) {
          throw new Error("metadata response exceeded the byte/chunk limit");
        }
        bytes += value.byteLength;
        text += decoder.decode(value, { stream: true });
      }
      text += decoder.decode();
      const value = parseCanonicalJsonText(text);
      complete = true;
      return value;
    } finally {
      if (!complete) cancelCatalogBody(reader || response.body);
      if (reader) {
        try { reader.releaseLock(); } catch (_) { /* Underlying abort still owns a pending read. */ }
      }
    }
  }

  const activeCatalogLoads = new WeakMap();

  // Remove old actions and callbacks whenever current catalog authority is unavailable.
  function renderCatalogState(documentObject, state) {
    for (const id of ["modelCatalogSearch", "modelCatalogAvailability"]) {
      const control = documentObject.getElementById(id);
      if (control) {
        control.disabled = true;
        control.oninput = null;
        control.onchange = null;
      }
    }
    const message = state === "error"
      ? "The model catalog is unavailable. Please try again later."
      : "Loading current catalog.";
    const list = documentObject.getElementById("modelCatalogList");
    if (list) {
      const empty = documentObject.createElement("li");
      empty.className = "catalog-empty";
      empty.textContent = message;
      list.replaceChildren(empty);
    }
    for (const id of ["modelCatalogTotal", "modelCatalogActive", "modelCatalogReady", "modelCatalogVisibleCount"]) {
      const node = documentObject.getElementById(id);
      if (node) node.textContent = state === "error" ? "Unavailable" : "Loading";
    }
    const status = documentObject.getElementById("modelCatalogStatus");
    if (status) {
      status.dataset.state = state;
      status.textContent = message;
    }
  }

  async function loadCatalog(documentObject, fetchImplementation = globalObject.fetch, options = {}) {
    const operation = new AbortController();
    activeCatalogLoads.get(documentObject)?.abort(new Error("Catalog load was superseded"));
    activeCatalogLoads.set(documentObject, operation);
    renderCatalogState(documentObject, "loading");
    let timer;
    let callerSignal;
    const cancelled = () => operation.abort(callerSignal.reason || new Error("Catalog load was cancelled"));
    try {
      const timeout = options.timeoutMilliseconds === undefined ? DEFAULT_LOAD_TIMEOUT_MILLISECONDS : options.timeoutMilliseconds;
      if (!Number.isSafeInteger(timeout) || timeout < 1 || timeout > MAX_LOAD_TIMEOUT_MILLISECONDS) throw new Error("Catalog timeout is invalid");
      callerSignal = options.signal;
      if (callerSignal !== undefined) {
        if (!callerSignal || typeof callerSignal.aborted !== "boolean" || typeof callerSignal.addEventListener !== "function" ||
            typeof callerSignal.removeEventListener !== "function") throw new Error("Catalog signal is invalid");
        callerSignal.addEventListener("abort", cancelled, { once: true });
        if (callerSignal.aborted) cancelled();
      }
      throwIfAborted(operation.signal);
      timer = setTimeout(() => operation.abort(new Error("Catalog load timed out")), timeout);
      if (typeof fetchImplementation !== "function") {
        throw new Error("catalog fetch is unavailable");
      }
      const base = new URL(globalObject.location ? globalObject.location.href : "https://minimodel.org/");
      const [catalog, peerFeed] = await Promise.all([
        fetchJson(new URL("/catalog/compositions.v1.json", base), fetchImplementation, operation.signal),
        fetchJson(new URL("/catalog/peer-announcements.v1.json", base), fetchImplementation, operation.signal),
      ]);
      const views = await buildVerifiedCatalogView(catalog, peerFeed, { ...options, signal: operation.signal });
      throwIfAborted(operation.signal);
      if (activeCatalogLoads.get(documentObject) !== operation) throw new Error("Catalog load was superseded");
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
    } catch (error) {
      operation.abort(error);
      if (activeCatalogLoads.get(documentObject) === operation) renderCatalogState(documentObject, "error");
      throw error;
    } finally {
      clearTimeout(timer);
      if (callerSignal && typeof callerSignal.removeEventListener === "function") callerSignal.removeEventListener("abort", cancelled);
      if (activeCatalogLoads.get(documentObject) === operation) activeCatalogLoads.delete(documentObject);
    }
  }

  const api = Object.freeze({
    CATALOG_SCHEMA,
    PEER_SCHEMA,
    BROWSER_ADMISSION_SCHEMA,
    QUALIFICATION_RECEIPT_SCHEMA,
    PEER_PROOF_SCHEMA,
    DIRECT_COMPOSITION_PROTOCOL_PROFILE,
    MEMBER_NAMES,
    MAX_METADATA_BYTES,
    MAX_BROWSER_ADMISSION_BYTES,
    MAX_QUALIFICATION_RECEIPT_BYTES,
    parseCanonicalJsonText,
    validateCatalog,
    validatePeerFeed,
    buildCatalogView,
    buildVerifiedCatalogView,
    renderCatalog,
    loadCatalog,
  });
  globalObject.MiniModelCatalog = api;

  if (globalObject.document) {
    // loadCatalog renders failures before rejecting; startup consumes that handled rejection.
    const start = () => loadCatalog(globalObject.document).catch(() => {});
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
