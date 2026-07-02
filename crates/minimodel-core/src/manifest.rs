use std::collections::BTreeMap;

use crate::sha256::normalize_sha256;

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
        self.validate_signature_placeholder()?;
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

    fn validate_signature_placeholder(&self) -> Result<(), String> {
        let kind = self.require("signature.kind")?;
        if kind == "unsigned-draft" {
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
        if self.had_comment {
            return Err("signed manifests must not contain comments".to_string());
        }
        self.require("signature.key_id")?;
        require_route(
            self.require("signature.public_key_route")?,
            "signature.public_key_route",
        )?;
        normalize_sha256(self.require("signature.payload_sha256")?)?;
        self.require("signature.value")?;
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
}
