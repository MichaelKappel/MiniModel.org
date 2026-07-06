use std::collections::BTreeMap;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;

use crate::sha256::{normalize_sha256, sha256_prefixed};

const SIGNATURE_KIND_UNSIGNED_DRAFT: &str = "unsigned-draft";
const SIGNATURE_KIND_EXTERNAL_V0: &str = "external-signature-v0";
const SIGNATURE_ALGORITHM_ED25519: &str = "ed25519";
const SIGNATURE_VALUE_ED25519_BASE64_PREFIX: &str = "ed25519-base64:";
const PUBLIC_KEY_KIND_V0: &str = "minimodel.public_key.v0";
const PUBLIC_KEY_USAGE_MANIFEST_SIGNING_V0: &str = "minimodel-manifest-signing-v0";

const REQUIRED_FIELDS: &[&str] = &[
    "manifest.version",
    "manifest.kind",
    "manifest.schema_id",
    "manifest.schema_checksum",
    "manifest.created_utc",
    "model.id",
    "model.version",
    "publisher.id",
    "publisher.key_id",
    "model_card.route",
    "license.route",
    "artifact.kind",
    "artifact.byte_count",
    "artifact.sha256",
    "artifact.acquisition",
    "artifact.project_server_url",
    "slm.format_version",
    "slm.model_shape",
    "slm.quantization",
    "slm.tokenizer_checksum",
    "slm.tensor_layout_checksum",
    "runtime.compatibility",
    "runtime.minimum_version",
    "source.kind",
    "source.id",
    "source.revision",
    "evidence.admission.status",
    "signature.kind",
];

const EVIDENCE_PAIRS: &[(&str, &str)] = &[
    ("source.config.route", "source.config.sha256"),
    ("source.tokenizer.route", "source.tokenizer.sha256"),
    (
        "evidence.source_review.route",
        "evidence.source_review.sha256",
    ),
    (
        "evidence.source_validation.route",
        "evidence.source_validation.sha256",
    ),
    (
        "evidence.runtime_smoke.route",
        "evidence.runtime_smoke.sha256",
    ),
    ("evidence.eval.route", "evidence.eval.sha256"),
    ("evidence.admission.route", "evidence.admission.sha256"),
];

#[derive(Clone, Debug)]
pub struct Manifest {
    fields: BTreeMap<String, String>,
    had_comment: bool,
}

#[derive(Clone, Debug)]
pub struct ManifestSignatureVerification {
    pub signature_algorithm: String,
    pub key_id: String,
    pub publisher_id: String,
    pub public_key_route: String,
    pub public_key_sha256: String,
    pub payload_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicKeyDocument {
    key_kind: String,
    key_algorithm: String,
    key_id: String,
    publisher_id: String,
    public_key_route: String,
    public_key_base64: String,
    public_key_sha256: String,
    usage: String,
    created_utc: String,
    revoked: bool,
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut fields = BTreeMap::new();
        let mut had_comment = false;
        for (line_index, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('#') {
                had_comment = true;
                continue;
            }
            let (raw_key, raw_value) = line
                .split_once('=')
                .ok_or_else(|| format!("line {} is missing '='", line_index + 1))?;
            let key = raw_key.trim();
            let value = raw_value.trim();
            validate_key(key).map_err(|error| format!("line {}: {error}", line_index + 1))?;
            validate_value(value).map_err(|error| format!("line {}: {error}", line_index + 1))?;
            if fields.insert(key.to_string(), value.to_string()).is_some() {
                return Err(format!("duplicate manifest key {key}"));
            }
        }
        Ok(Self {
            fields,
            had_comment,
        })
    }

    pub fn require(&self, key: &str) -> Result<&str, String> {
        self.fields
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| format!("missing required manifest field {key}"))
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    pub fn fields(&self) -> &BTreeMap<String, String> {
        &self.fields
    }

    /// Returns the canonical sorted manifest body covered by a signature payload digest.
    pub fn canonical_signature_payload(&self) -> String {
        let mut payload = String::new();
        for (key, value) in self
            .fields
            .iter()
            .filter(|(key, _)| !key.starts_with("signature."))
        {
            payload.push_str(key);
            payload.push('=');
            payload.push_str(value);
            payload.push('\n');
        }
        payload
    }

    /// Returns the SHA-256 of the canonical non-signature manifest body.
    pub fn canonical_signature_payload_sha256(&self) -> String {
        sha256_prefixed(self.canonical_signature_payload().as_bytes())
    }

    pub fn verify_external_signature_key_json(
        &self,
        public_key_json: &str,
    ) -> Result<ManifestSignatureVerification, String> {
        self.validate()?;
        let kind = self.require("signature.kind")?;
        if kind != SIGNATURE_KIND_EXTERNAL_V0 {
            return Err(format!(
                "signed verification requires signature.kind={SIGNATURE_KIND_EXTERNAL_V0}"
            ));
        }
        let document: PublicKeyDocument = serde_json::from_str(public_key_json)
            .map_err(|source| format!("invalid public key JSON: {source}"))?;
        document.validate_for_manifest(self)?;
        let public_key_bytes =
            decode_base64_array::<32>(&document.public_key_base64, "public_key_base64")?;
        let actual_public_key_sha256 = sha256_prefixed(&public_key_bytes);
        let expected_public_key_sha256 = normalize_sha256(&document.public_key_sha256)?;
        if actual_public_key_sha256 != expected_public_key_sha256 {
            return Err(format!(
                "public_key_sha256 mismatch: expected {expected_public_key_sha256}, public key bytes had {actual_public_key_sha256}"
            ));
        }
        let signature = parse_ed25519_signature_value(self.require("signature.value")?)?;
        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
            .map_err(|source| format!("invalid Ed25519 public key: {source}"))?;
        verifying_key
            .verify(self.canonical_signature_payload().as_bytes(), &signature)
            .map_err(|_| {
                "signature.value did not verify the canonical manifest payload".to_string()
            })?;
        Ok(ManifestSignatureVerification {
            signature_algorithm: SIGNATURE_ALGORITHM_ED25519.to_string(),
            key_id: document.key_id,
            publisher_id: document.publisher_id,
            public_key_route: document.public_key_route,
            public_key_sha256: actual_public_key_sha256,
            payload_sha256: self.canonical_signature_payload_sha256(),
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        for key in REQUIRED_FIELDS {
            self.require(key)?;
        }
        require_equal(self, "manifest.version", "0")?;
        require_equal(self, "manifest.kind", "minimodel.manifest")?;
        require_equal(self, "manifest.schema_id", "minimodel.manifest.v0")?;
        normalize_sha256(self.require("manifest.schema_checksum")?)?;
        require_timestampish(
            self.require("manifest.created_utc")?,
            "manifest.created_utc",
        )?;
        require_route_safe(self.require("model.id")?, "model.id")?;
        require_route_safe(self.require("publisher.id")?, "publisher.id")?;
        require_equal(self, "artifact.kind", "slm")?;
        require_decimal(self.require("artifact.byte_count")?, "artifact.byte_count")?;
        normalize_sha256(self.require("artifact.sha256")?)?;
        require_member(
            self.require("artifact.acquisition")?,
            "artifact.acquisition",
            &[
                "user-local-file",
                "user-external-download",
                "consent-peer-transfer",
            ],
        )?;
        require_equal(self, "artifact.project_server_url", "none")?;
        require_equal(self, "slm.format_version", "1")?;
        require_member(
            self.require("slm.quantization")?,
            "slm.quantization",
            &["f32", "q8_0", "q4_0"],
        )?;
        require_hex64(
            self.require("slm.tokenizer_checksum")?,
            "slm.tokenizer_checksum",
        )?;
        require_hex64(
            self.require("slm.tensor_layout_checksum")?,
            "slm.tensor_layout_checksum",
        )?;
        if let Some(value) = self.get("slm.checksum") {
            require_hex64(value, "slm.checksum")?;
        }
        require_equal(self, "runtime.compatibility", "tinyrustlm")?;
        require_member(
            self.require("source.kind")?,
            "source.kind",
            &["safetensors", "slm-native", "synthetic", "unknown"],
        )?;
        require_member(
            self.require("evidence.admission.status")?,
            "evidence.admission.status",
            &["passed", "pending", "unavailable"],
        )?;
        self.validate_routes()?;
        self.validate_evidence_pairs()?;
        self.validate_source_discovery()?;
        self.validate_signature()?;
        self.validate_chunk_metadata()?;
        self.validate_boundary_scan()?;
        Ok(())
    }

    fn validate_routes(&self) -> Result<(), String> {
        for key in ["model_card.route", "license.route"] {
            let route = self.require(key)?;
            require_route(route, key)?;
        }
        for key in self.fields.keys().filter(|key| key.ends_with(".route")) {
            let route = self.require(key)?;
            if route != "none" {
                require_route(route, key)?;
            }
        }
        Ok(())
    }

    fn validate_evidence_pairs(&self) -> Result<(), String> {
        let status = self.require("evidence.admission.status")?;
        for (route_key, sha_key) in EVIDENCE_PAIRS {
            let route = self.get(route_key);
            let sha = self.get(sha_key);
            if sha.is_some() && route.is_none() {
                return Err(format!("{sha_key} is present without {route_key}"));
            }
            if route.is_some() && sha.is_none() && status == "passed" {
                return Err(format!("{route_key} is present without {sha_key}"));
            }
            if let Some(route) = route {
                require_route(route, route_key)?;
            }
            if let Some(sha) = sha {
                normalize_sha256(sha).map_err(|_| format!("{sha_key} is not a SHA-256 value"))?;
            }
        }
        if status == "passed" {
            self.require("evidence.admission.route")?;
            normalize_sha256(self.require("evidence.admission.sha256")?)
                .map_err(|_| "passed admission evidence requires a SHA-256 checksum".to_string())?;
        }
        Ok(())
    }

    fn validate_source_discovery(&self) -> Result<(), String> {
        if let Some(kind) = self.get("source.discovery.kind") {
            require_member(
                kind,
                "source.discovery.kind",
                &[
                    "local-file",
                    "local-list",
                    "huggingface-hub",
                    "minimodel-p2p",
                    "unknown",
                ],
            )?;
        }
        if let Some(route) = self.get("source.discovery.route") {
            require_route(route, "source.discovery.route")?;
        }
        if let Some(value) = self.get("source.discovery.user_token_required") {
            require_member(
                value,
                "source.discovery.user_token_required",
                &["true", "false"],
            )?;
        }
        Ok(())
    }

    fn validate_signature(&self) -> Result<(), String> {
        let kind = self.require("signature.kind")?;
        if kind == SIGNATURE_KIND_UNSIGNED_DRAFT {
            if let Some(value) = self.get("signature.key_id") {
                require_member(value, "signature.key_id", &["none"])?;
            }
            if let Some(value) = self.get("signature.value") {
                require_member(value, "signature.value", &["none"])?;
            }
            if let Some(value) = self.get("signature.payload_sha256") {
                if value != "none" {
                    normalize_sha256(value)?;
                }
            }
            return Ok(());
        }
        if kind != SIGNATURE_KIND_EXTERNAL_V0 {
            return Err(format!("signature.kind has unsupported value {kind}"));
        }
        if self.had_comment {
            return Err("signed manifests must not contain comments".to_string());
        }
        let key_id = self.require("signature.key_id")?;
        if key_id.is_empty() || key_id == "none" {
            return Err("signature.key_id must name a signing key".to_string());
        }
        let public_key_route = self.require("signature.public_key_route")?;
        if public_key_route == "none" {
            return Err("signature.public_key_route must name public key metadata".to_string());
        }
        require_route(public_key_route, "signature.public_key_route")?;
        let expected_payload_sha256 = normalize_sha256(self.require("signature.payload_sha256")?)?;
        let actual_payload_sha256 = self.canonical_signature_payload_sha256();
        if expected_payload_sha256 != actual_payload_sha256 {
            return Err(format!(
                "signature.payload_sha256 mismatch: expected {expected_payload_sha256}, canonical payload had {actual_payload_sha256}"
            ));
        }
        let signature_value = self.require("signature.value")?;
        if signature_value.is_empty() || signature_value == "none" {
            return Err("signature.value must contain an external signature value".to_string());
        }
        Ok(())
    }

    fn validate_chunk_metadata(&self) -> Result<(), String> {
        let mode = self.get("chunks.mode").unwrap_or("none");
        match mode {
            "none" => Ok(()),
            "fixed-size-merkle-v0" => {
                require_decimal(self.require("chunks.size")?, "chunks.size")?;
                require_decimal(self.require("chunks.count")?, "chunks.count")?;
                normalize_sha256(self.require("chunks.merkle_root_sha256")?)?;
                if let Some(route) = self.get("chunks.list.route") {
                    require_route(route, "chunks.list.route")?;
                    normalize_sha256(self.get("chunks.list.sha256").ok_or_else(|| {
                        "chunks.list.route is present without chunks.list.sha256".to_string()
                    })?)?;
                }
                Ok(())
            }
            _ => Err(format!("unsupported chunks.mode {mode}")),
        }
    }

    fn validate_boundary_scan(&self) -> Result<(), String> {
        for (key, value) in &self.fields {
            let haystack = format!("{key}={value}").to_ascii_lowercase();
            for forbidden in [
                "/resolve/",
                "inference-endpoint",
                "inference_endpoint",
                "inference provider",
                "inference_provider",
                "remote_inference",
                "server_execution",
                "server-side execution",
                "automatic_browser_fetch",
                "install_command",
                "post_load",
                "postload",
                "script_hook",
                "execution_hook",
                "space_url",
            ] {
                if haystack.contains(forbidden) {
                    return Err(format!(
                        "manifest field {key} contains forbidden remote execution or model-byte marker {forbidden}"
                    ));
                }
            }
            if haystack.contains("project-owned") && haystack.contains("token") {
                return Err(format!("{key} requires a project-owned token"));
            }
            if key.contains("project_token") && value.eq_ignore_ascii_case("true") {
                return Err(format!("{key} requires a project-owned token"));
            }
            if (key.contains("server_serves_model_bytes")
                || key.contains("server_runs_model_bytes")
                || key.contains("project_server_serves_model_bytes")
                || key.contains("project_server_runs_model_bytes"))
                && value.eq_ignore_ascii_case("true")
            {
                return Err(format!("{key} violates the metadata-only boundary"));
            }
        }
        Ok(())
    }
}

impl PublicKeyDocument {
    fn validate_for_manifest(&self, manifest: &Manifest) -> Result<(), String> {
        validate_public_key_value(&self.key_kind, "key_kind")?;
        validate_public_key_value(&self.key_algorithm, "key_algorithm")?;
        validate_public_key_value(&self.key_id, "key_id")?;
        validate_public_key_value(&self.publisher_id, "publisher_id")?;
        validate_public_key_value(&self.public_key_route, "public_key_route")?;
        validate_public_key_value(&self.public_key_base64, "public_key_base64")?;
        validate_public_key_value(&self.public_key_sha256, "public_key_sha256")?;
        validate_public_key_value(&self.usage, "usage")?;
        validate_public_key_value(&self.created_utc, "created_utc")?;
        if self.key_kind != PUBLIC_KEY_KIND_V0 {
            return Err(format!("key_kind must be {PUBLIC_KEY_KIND_V0}"));
        }
        if self.key_algorithm != SIGNATURE_ALGORITHM_ED25519 {
            return Err(format!(
                "key_algorithm must be {SIGNATURE_ALGORITHM_ED25519}"
            ));
        }
        if self.usage != PUBLIC_KEY_USAGE_MANIFEST_SIGNING_V0 {
            return Err(format!(
                "usage must be {PUBLIC_KEY_USAGE_MANIFEST_SIGNING_V0}"
            ));
        }
        require_timestampish(&self.created_utc, "created_utc")?;
        require_route_safe(&self.key_id, "key_id")?;
        require_route_safe(&self.publisher_id, "publisher_id")?;
        require_route(&self.public_key_route, "public_key_route")?;
        normalize_sha256(&self.public_key_sha256)?;
        if self.revoked {
            return Err("public key metadata is revoked".to_string());
        }
        let manifest_key_id = manifest.require("signature.key_id")?;
        if self.key_id != manifest_key_id {
            return Err(format!(
                "public key key_id {} does not match manifest signature.key_id {manifest_key_id}",
                self.key_id
            ));
        }
        let manifest_publisher_id = manifest.require("publisher.id")?;
        if self.publisher_id != manifest_publisher_id {
            return Err(format!(
                "public key publisher_id {} does not match manifest publisher.id {manifest_publisher_id}",
                self.publisher_id
            ));
        }
        let manifest_public_key_route = manifest.require("signature.public_key_route")?;
        if self.public_key_route != manifest_public_key_route {
            return Err(format!(
                "public key route {} does not match manifest signature.public_key_route {manifest_public_key_route}",
                self.public_key_route
            ));
        }
        Ok(())
    }
}

fn validate_public_key_value(value: &str, key: &str) -> Result<(), String> {
    validate_value(value).map_err(|source| format!("{key}: {source}"))
}

fn decode_base64_array<const N: usize>(value: &str, key: &str) -> Result<[u8; N], String> {
    let bytes = BASE64_STANDARD
        .decode(value)
        .map_err(|source| format!("{key} is not base64: {source}"))?;
    bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| format!("{key} must decode to {N} bytes, got {}", bytes.len()))
}

fn parse_ed25519_signature_value(value: &str) -> Result<Signature, String> {
    let encoded = value
        .strip_prefix(SIGNATURE_VALUE_ED25519_BASE64_PREFIX)
        .ok_or_else(|| {
            format!("signature.value must start with {SIGNATURE_VALUE_ED25519_BASE64_PREFIX}")
        })?;
    let signature_bytes = decode_base64_array::<64>(encoded, "signature.value")?;
    Ok(Signature::from_bytes(&signature_bytes))
}

fn require_equal(manifest: &Manifest, key: &str, expected: &str) -> Result<(), String> {
    let actual = manifest.require(key)?;
    if actual != expected {
        return Err(format!("{key} must be {expected}, got {actual}"));
    }
    Ok(())
}

fn require_member(value: &str, key: &str, allowed: &[&str]) -> Result<(), String> {
    if allowed.contains(&value) {
        return Ok(());
    }
    Err(format!("{key} has unsupported value {value}"))
}

fn require_decimal(value: &str, key: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{key} must be a decimal integer"))?;
    if parsed == 0 {
        return Err(format!("{key} must be greater than zero"));
    }
    Ok(parsed)
}

fn require_hex64(value: &str, key: &str) -> Result<u64, String> {
    let hex = value
        .strip_prefix("0x")
        .ok_or_else(|| format!("{key} must start with 0x"))?;
    if hex.len() != 16 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{key} must be 0x plus 16 hex characters"));
    }
    u64::from_str_radix(hex, 16).map_err(|_| format!("{key} contains invalid hex"))
}

fn require_route_safe(value: &str, key: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{key} must not be empty"));
    }
    if value.bytes().all(|byte| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(byte, b'-' | b'_' | b'.' | b'/')
    }) {
        Ok(())
    } else {
        Err(format!("{key} must be route-safe lowercase ASCII"))
    }
}

fn require_route(route: &str, key: &str) -> Result<(), String> {
    if route.is_empty() {
        return Err(format!("{key} must not be empty"));
    }
    validate_value(route)?;
    if is_model_byte_route(route) {
        return Err(format!("{key} must not point at model bytes"));
    }
    Ok(())
}

fn require_timestampish(value: &str, key: &str) -> Result<(), String> {
    if value.len() >= 20 && value.contains('T') && value.ends_with('Z') {
        Ok(())
    } else {
        Err(format!("{key} must be an ISO-like UTC timestamp"))
    }
}

fn validate_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("manifest key must not be empty".to_string());
    }
    if key.starts_with('.') || key.ends_with('.') || key.contains("..") {
        return Err("manifest key dot separators are invalid".to_string());
    }
    if key.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_')
    }) {
        Ok(())
    } else {
        Err(format!("manifest key {key} must be lowercase ASCII"))
    }
}

fn validate_value(value: &str) -> Result<(), String> {
    if value.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return Err("manifest values must not contain control characters".to_string());
    }
    if !value.is_ascii() {
        return Err("manifest values must be ASCII in v0".to_string());
    }
    Ok(())
}

fn is_model_byte_route(route: &str) -> bool {
    let lower = route.to_ascii_lowercase();
    if lower.contains("/resolve/") {
        return true;
    }
    for suffix in [
        ".slm",
        ".safetensors",
        ".bin",
        ".gguf",
        ".pt",
        ".pth",
        ".onnx",
        ".ckpt",
        ".zip",
        ".tar",
        ".tar.gz",
    ] {
        if lower.ends_with(suffix) || lower.contains(&format!("{suffix}?")) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn signed_manifest_and_key_json() -> (Manifest, String) {
        let mut manifest = valid_manifest();
        manifest.fields.insert(
            "signature.kind".to_string(),
            SIGNATURE_KIND_EXTERNAL_V0.to_string(),
        );
        manifest.fields.insert(
            "signature.key_id".to_string(),
            "minimodel-development-key-v0".to_string(),
        );
        manifest.fields.insert(
            "signature.public_key_route".to_string(),
            "../keys/minimodel-development-key-v0.public-key.json".to_string(),
        );
        let payload_sha256 = manifest.canonical_signature_payload_sha256();
        manifest
            .fields
            .insert("signature.payload_sha256".to_string(), payload_sha256);
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let verifying_key = signing_key.verifying_key();
        let signature = signing_key.sign(manifest.canonical_signature_payload().as_bytes());
        manifest.fields.insert(
            "signature.value".to_string(),
            format!(
                "{SIGNATURE_VALUE_ED25519_BASE64_PREFIX}{}",
                BASE64_STANDARD.encode(signature.to_bytes())
            ),
        );
        let public_key_bytes = verifying_key.to_bytes();
        let key_json = format!(
            r#"{{
  "key_kind": "minimodel.public_key.v0",
  "key_algorithm": "ed25519",
  "key_id": "minimodel-development-key-v0",
  "publisher_id": "minimodel.org",
  "public_key_route": "../keys/minimodel-development-key-v0.public-key.json",
  "public_key_base64": "{}",
  "public_key_sha256": "{}",
  "usage": "minimodel-manifest-signing-v0",
  "created_utc": "2026-07-06T00:00:00Z",
  "revoked": false
}}"#,
            BASE64_STANDARD.encode(public_key_bytes),
            sha256_prefixed(&public_key_bytes)
        );
        (manifest, key_json)
    }

    fn valid_manifest() -> Manifest {
        Manifest::parse(
            r#"
manifest.version=0
manifest.kind=minimodel.manifest
manifest.schema_id=minimodel.manifest.v0
manifest.schema_checksum=sha256:3333333333333333333333333333333333333333333333333333333333333333
manifest.created_utc=2026-07-02T00:00:00Z
model.id=tinyrustlm/tiny-test-model-q8
model.version=0.1.0-fixture
publisher.id=minimodel.org
publisher.key_id=minimodel-development-key-v0
model_card.route=../docs/catalog-boundary.md
license.route=../LICENSE.txt
artifact.kind=slm
artifact.byte_count=8832
artifact.sha256=sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf
artifact.acquisition=user-local-file
artifact.project_server_url=none
slm.format_version=1
slm.checksum=0x481f1037d57ebcd3
slm.model_shape=vocab=260,hidden=8,layers=1,heads=2,kv_heads=2,head_dim=4,ffn=16,context=16
slm.quantization=q8_0
slm.tokenizer_checksum=0x288f65d94c26743e
slm.tensor_layout_checksum=0xd8fb6e636a8badd6
runtime.compatibility=tinyrustlm
runtime.minimum_version=0.1.0
source.kind=synthetic
source.id=tinyrustlm-fixture
source.revision=local-fixture-2026-07-02
evidence.admission.status=passed
evidence.admission.route=../examples/tiny-test-model-q8.minimodel-peer-offer
evidence.admission.sha256=sha256:9da5ada1eb8dd9bb77b8e46cea624f380808aee82568f555b6420fd59fa8d642
chunks.mode=none
signature.kind=unsigned-draft
"#,
        )
        .unwrap()
    }

    #[test]
    fn validates_minimal_manifest() {
        valid_manifest().validate().unwrap();
    }

    #[test]
    fn rejects_duplicate_keys() {
        let error = Manifest::parse("manifest.version=0\nmanifest.version=1\n").unwrap_err();
        assert!(error.contains("duplicate"));
    }

    #[test]
    fn rejects_project_server_model_url() {
        let mut manifest = valid_manifest();
        manifest.fields.insert(
            "artifact.project_server_url".to_string(),
            "https://example.com/model.slm".to_string(),
        );
        let error = manifest.validate().unwrap_err();
        assert!(error.contains("artifact.project_server_url"));
    }

    #[test]
    fn rejects_hugging_face_resolve_url() {
        let mut manifest = valid_manifest();
        manifest.fields.insert(
            "source.discovery.route".to_string(),
            "https://huggingface.co/x/y/resolve/main/model.safetensors".to_string(),
        );
        let error = manifest.validate().unwrap_err();
        assert!(error.contains("model bytes") || error.contains("/resolve/"));
    }

    #[test]
    fn rejects_passed_evidence_without_checksum() {
        let mut manifest = valid_manifest();
        manifest.fields.remove("evidence.admission.sha256");
        let error = manifest.validate().unwrap_err();
        assert!(error.contains("evidence.admission"));
    }

    #[test]
    fn validates_external_signature_payload_digest() {
        let mut manifest = valid_manifest();
        manifest.fields.insert(
            "signature.kind".to_string(),
            SIGNATURE_KIND_EXTERNAL_V0.to_string(),
        );
        manifest.fields.insert(
            "signature.key_id".to_string(),
            "minimodel-development-key-v0".to_string(),
        );
        manifest.fields.insert(
            "signature.public_key_route".to_string(),
            "../keys/minimodel-development-key-v0.pub".to_string(),
        );
        manifest.fields.insert(
            "signature.value".to_string(),
            "test-signature-value".to_string(),
        );
        let payload_sha256 = manifest.canonical_signature_payload_sha256();
        manifest
            .fields
            .insert("signature.payload_sha256".to_string(), payload_sha256);

        manifest.validate().unwrap();
    }

    #[test]
    fn rejects_external_signature_payload_digest_drift() {
        let mut manifest = valid_manifest();
        manifest.fields.insert(
            "signature.kind".to_string(),
            SIGNATURE_KIND_EXTERNAL_V0.to_string(),
        );
        manifest.fields.insert(
            "signature.key_id".to_string(),
            "minimodel-development-key-v0".to_string(),
        );
        manifest.fields.insert(
            "signature.public_key_route".to_string(),
            "../keys/minimodel-development-key-v0.pub".to_string(),
        );
        manifest.fields.insert(
            "signature.value".to_string(),
            "test-signature-value".to_string(),
        );
        let payload_sha256 = manifest.canonical_signature_payload_sha256();
        manifest
            .fields
            .insert("signature.payload_sha256".to_string(), payload_sha256);
        manifest
            .fields
            .insert("model.version".to_string(), "0.1.1-drift".to_string());

        let error = manifest.validate().unwrap_err();
        assert!(error.contains("signature.payload_sha256 mismatch"));
    }

    #[test]
    fn rejects_unknown_signature_kind() {
        let mut manifest = valid_manifest();
        manifest
            .fields
            .insert("signature.kind".to_string(), "ed25519".to_string());
        manifest.fields.insert(
            "signature.key_id".to_string(),
            "minimodel-development-key-v0".to_string(),
        );
        manifest.fields.insert(
            "signature.public_key_route".to_string(),
            "../keys/minimodel-development-key-v0.pub".to_string(),
        );
        let payload_sha256 = manifest.canonical_signature_payload_sha256();
        manifest
            .fields
            .insert("signature.payload_sha256".to_string(), payload_sha256);
        manifest.fields.insert(
            "signature.value".to_string(),
            "test-signature-value".to_string(),
        );

        let error = manifest.validate().unwrap_err();
        assert!(error.contains("unsupported value ed25519"));
    }

    #[test]
    fn verifies_external_ed25519_signature() {
        let (manifest, key_json) = signed_manifest_and_key_json();

        let report = manifest
            .verify_external_signature_key_json(&key_json)
            .unwrap();

        assert_eq!(report.signature_algorithm, "ed25519");
        assert_eq!(report.key_id, "minimodel-development-key-v0");
        assert_eq!(report.publisher_id, "minimodel.org");
        assert_eq!(
            report.public_key_route,
            "../keys/minimodel-development-key-v0.public-key.json"
        );
        assert_eq!(
            report.payload_sha256,
            manifest.canonical_signature_payload_sha256()
        );
    }

    #[test]
    fn rejects_external_ed25519_signature_payload_drift() {
        let (mut manifest, key_json) = signed_manifest_and_key_json();
        manifest
            .fields
            .insert("model.version".to_string(), "0.1.1-drift".to_string());
        manifest.fields.insert(
            "signature.payload_sha256".to_string(),
            manifest.canonical_signature_payload_sha256(),
        );

        let error = manifest
            .verify_external_signature_key_json(&key_json)
            .unwrap_err();

        assert!(error.contains("signature.value did not verify"));
    }

    #[test]
    fn rejects_external_ed25519_public_key_drift() {
        let (manifest, key_json) = signed_manifest_and_key_json();
        let mut key_value: serde_json::Value = serde_json::from_str(&key_json).unwrap();
        key_value["public_key_sha256"] = serde_json::Value::String(
            "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        );
        let drifted_key_json = serde_json::to_string(&key_value).unwrap();

        let error = manifest
            .verify_external_signature_key_json(&drifted_key_json)
            .unwrap_err();

        assert!(error.contains("public_key_sha256 mismatch"));
    }

    #[test]
    fn rejects_revoked_external_ed25519_public_key() {
        let (manifest, key_json) = signed_manifest_and_key_json();
        let revoked_key_json = key_json.replace("\"revoked\": false", "\"revoked\": true");

        let error = manifest
            .verify_external_signature_key_json(&revoked_key_json)
            .unwrap_err();

        assert!(error.contains("revoked"));
    }
}
