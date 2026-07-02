mod manifest;
mod sha256;
mod slm;

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub use manifest::Manifest;
pub use slm::SlmReport;

use sha256::{bytes_to_hex, normalize_sha256, parse_sha256_hex, sha256, sha256_prefixed};

#[derive(Debug, Clone)]
pub struct ArtifactVerification {
    pub manifest_sha256: String,
    pub manifest_model_id: String,
    pub artifact_byte_count: u64,
    pub artifact_sha256: String,
    pub evidence_status: String,
    pub slm: SlmReport,
}

pub fn verify_manifest_path(path: &Path) -> Result<Manifest, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read manifest: {error}"))?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| "manifest must be valid UTF-8".to_string())?;
    let manifest = Manifest::parse(text)?;
    manifest.validate()?;
    Ok(manifest)
}

pub fn verify_artifact_path(
    manifest_path: &Path,
    artifact_path: &Path,
) -> Result<ArtifactVerification, String> {
    let manifest_bytes =
        fs::read(manifest_path).map_err(|error| format!("failed to read manifest: {error}"))?;
    let manifest_text = std::str::from_utf8(&manifest_bytes)
        .map_err(|_| "manifest must be valid UTF-8".to_string())?;
    let manifest = Manifest::parse(manifest_text)?;
    manifest.validate()?;
    let artifact_bytes =
        fs::read(artifact_path).map_err(|error| format!("failed to read artifact: {error}"))?;
    verify_artifact_bytes(&manifest, &manifest_bytes, &artifact_bytes)
}

pub fn verify_artifact_bytes(
    manifest: &Manifest,
    manifest_bytes: &[u8],
    artifact_bytes: &[u8],
) -> Result<ArtifactVerification, String> {
    let expected_byte_count = manifest
        .require("artifact.byte_count")?
        .parse::<u64>()
        .map_err(|_| "artifact.byte_count must be a decimal integer".to_string())?;
    if artifact_bytes.len() as u64 != expected_byte_count {
        return Err(format!(
            "artifact byte count mismatch: manifest expected {expected_byte_count}, file had {}",
            artifact_bytes.len()
        ));
    }
    let expected_artifact_sha = normalize_sha256(manifest.require("artifact.sha256")?)?;
    let actual_artifact_sha = sha256_prefixed(artifact_bytes);
    if actual_artifact_sha != expected_artifact_sha {
        return Err(format!(
            "artifact SHA-256 mismatch: manifest expected {expected_artifact_sha}, file had {actual_artifact_sha}"
        ));
    }
    verify_chunks(manifest, artifact_bytes)?;
    let slm = slm::validate_slm_bytes(artifact_bytes)?;
    compare_slm_manifest(manifest, &slm)?;
    Ok(ArtifactVerification {
        manifest_sha256: sha256_prefixed(manifest_bytes),
        manifest_model_id: manifest.require("model.id")?.to_string(),
        artifact_byte_count: artifact_bytes.len() as u64,
        artifact_sha256: actual_artifact_sha,
        evidence_status: manifest.require("evidence.admission.status")?.to_string(),
        slm,
    })
}

pub fn write_import_receipt_path(
    manifest_path: &Path,
    artifact_path: &Path,
    receipt_path: &Path,
) -> Result<ArtifactVerification, String> {
    let report = verify_artifact_path(manifest_path, artifact_path)?;
    let receipt = import_receipt_text(&report, artifact_path)?;
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create receipt directory: {error}"))?;
    }
    fs::write(receipt_path, receipt)
        .map_err(|error| format!("failed to write import receipt: {error}"))?;
    Ok(report)
}

pub fn import_receipt_text(
    report: &ArtifactVerification,
    artifact_path: &Path,
) -> Result<String, String> {
    let file_name = artifact_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "artifact path must include a file name".to_string())?;
    let mut text = String::new();
    push_line(&mut text, "receipt.version", "0");
    push_line(&mut text, "receipt.kind", "minimodel.import_receipt");
    push_line(&mut text, "receipt.created_utc", &utc_now()?);
    push_line(&mut text, "manifest.sha256", &report.manifest_sha256);
    push_line(&mut text, "manifest.model_id", &report.manifest_model_id);
    push_line(&mut text, "artifact.local_path_hint", file_name);
    push_line(
        &mut text,
        "artifact.byte_count",
        &report.artifact_byte_count.to_string(),
    );
    push_line(&mut text, "artifact.sha256", &report.artifact_sha256);
    push_line(&mut text, "slm.validation_status", "passed");
    push_line(&mut text, "slm.model_shape", &report.slm.model_shape);
    push_line(&mut text, "slm.quantization", &report.slm.quantization);
    push_line(&mut text, "evidence.status", &report.evidence_status);
    push_line(
        &mut text,
        "import.next_gate",
        "user-selected-tinyrustlm-local-file-open",
    );
    Ok(text)
}

fn compare_slm_manifest(manifest: &Manifest, slm: &SlmReport) -> Result<(), String> {
    require_match(
        "slm.format_version",
        manifest.require("slm.format_version")?,
        &slm.format_version.to_string(),
    )?;
    require_match(
        "slm.model_shape",
        manifest.require("slm.model_shape")?,
        &slm.model_shape,
    )?;
    require_match(
        "slm.quantization",
        manifest.require("slm.quantization")?,
        &slm.quantization,
    )?;
    require_match(
        "slm.tokenizer_checksum",
        manifest.require("slm.tokenizer_checksum")?,
        &slm.tokenizer_checksum,
    )?;
    require_match(
        "slm.tensor_layout_checksum",
        manifest.require("slm.tensor_layout_checksum")?,
        &slm.tensor_layout_checksum,
    )?;
    if let Some(expected) = manifest.get("slm.checksum") {
        require_match("slm.checksum", expected, &slm.checksum)?;
    }
    Ok(())
}

fn require_match(field: &str, expected: &str, actual: &str) -> Result<(), String> {
    if expected.eq_ignore_ascii_case(actual) {
        return Ok(());
    }
    Err(format!(
        "{field} mismatch: manifest expected {expected}, artifact had {actual}"
    ))
}

#[derive(Debug, Clone)]
struct ChunkProof {
    offset: usize,
    byte_count: usize,
    sha256: String,
}

fn verify_chunks(manifest: &Manifest, artifact_bytes: &[u8]) -> Result<(), String> {
    let mode = manifest.get("chunks.mode").unwrap_or("none");
    if mode == "none" {
        return Ok(());
    }
    if mode != "fixed-size-merkle-v0" {
        return Err(format!("unsupported chunks.mode {mode}"));
    }
    let chunk_size = parse_usize(manifest.require("chunks.size")?, "chunks.size")?;
    if chunk_size == 0 {
        return Err("chunks.size must be greater than zero".to_string());
    }
    let expected_count = parse_usize(manifest.require("chunks.count")?, "chunks.count")?;
    let chunks = chunk_proofs(artifact_bytes, chunk_size)?;
    if chunks.len() != expected_count {
        return Err(format!(
            "chunk count mismatch: manifest expected {expected_count}, artifact produced {}",
            chunks.len()
        ));
    }
    let expected_root = normalize_sha256(manifest.require("chunks.merkle_root_sha256")?)?;
    let actual_root = merkle_root_sha256(&chunks)?;
    if actual_root != expected_root {
        return Err(format!(
            "chunk Merkle root mismatch: manifest expected {expected_root}, artifact produced {actual_root}"
        ));
    }
    if let Some(expected_list_sha) = manifest.get("chunks.list.sha256") {
        let expected_list_sha = normalize_sha256(expected_list_sha)?;
        let actual_list_sha = sha256_prefixed(chunk_list_text(&chunks).as_bytes());
        if actual_list_sha != expected_list_sha {
            return Err(format!(
                "chunk list SHA-256 mismatch: manifest expected {expected_list_sha}, artifact produced {actual_list_sha}"
            ));
        }
    }
    for (index, chunk) in chunks.iter().enumerate() {
        let offset_key = format!("chunk.{index}.offset");
        let count_key = format!("chunk.{index}.byte_count");
        let sha_key = format!("chunk.{index}.sha256");
        if let Some(expected) = manifest.get(&offset_key) {
            let expected = parse_usize(expected, &offset_key)?;
            if expected != chunk.offset {
                return Err(format!(
                    "{offset_key} mismatch: manifest expected {expected}, artifact produced {}",
                    chunk.offset
                ));
            }
        }
        if let Some(expected) = manifest.get(&count_key) {
            let expected = parse_usize(expected, &count_key)?;
            if expected != chunk.byte_count {
                return Err(format!(
                    "{count_key} mismatch: manifest expected {expected}, artifact produced {}",
                    chunk.byte_count
                ));
            }
        }
        if let Some(expected) = manifest.get(&sha_key) {
            let expected = normalize_sha256(expected)?;
            if expected != chunk.sha256 {
                return Err(format!(
                    "{sha_key} mismatch: manifest expected {expected}, artifact produced {}",
                    chunk.sha256
                ));
            }
        }
    }
    Ok(())
}

fn chunk_proofs(bytes: &[u8], chunk_size: usize) -> Result<Vec<ChunkProof>, String> {
    if chunk_size == 0 {
        return Err("chunk size must be greater than zero".to_string());
    }
    let mut chunks = Vec::new();
    for (index, chunk) in bytes.chunks(chunk_size).enumerate() {
        chunks.push(ChunkProof {
            offset: index
                .checked_mul(chunk_size)
                .ok_or_else(|| "chunk offset overflow".to_string())?,
            byte_count: chunk.len(),
            sha256: sha256_prefixed(chunk),
        });
    }
    if chunks.is_empty() {
        return Err("artifact must not be empty".to_string());
    }
    Ok(chunks)
}

fn chunk_list_text(chunks: &[ChunkProof]) -> String {
    let mut text = String::new();
    for (index, chunk) in chunks.iter().enumerate() {
        text.push_str(&format!(
            "{}={}={}={}\n",
            index, chunk.offset, chunk.byte_count, chunk.sha256
        ));
    }
    text
}

fn merkle_root_sha256(chunks: &[ChunkProof]) -> Result<String, String> {
    if chunks.is_empty() {
        return Err("chunk Merkle tree requires at least one chunk".to_string());
    }
    let mut level = chunks
        .iter()
        .map(|chunk| parse_sha256_hex(&chunk.sha256))
        .collect::<Result<Vec<_>, _>>()?;
    while level.len() > 1 {
        let mut next = Vec::new();
        for pair in level.chunks(2) {
            let left = &pair[0];
            let right = pair.get(1).unwrap_or(left);
            let mut bytes = Vec::with_capacity(64);
            bytes.extend_from_slice(left);
            bytes.extend_from_slice(right);
            next.push(sha256(&bytes));
        }
        level = next;
    }
    Ok(format!("sha256:{}", bytes_to_hex(&level[0])))
}

fn parse_usize(value: &str, field: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("{field} must be a decimal integer"))
}

fn push_line(text: &mut String, key: &str, value: &str) {
    text.push_str(key);
    text.push('=');
    text.push_str(value);
    text.push('\n');
}

fn utc_now() -> Result<String, String> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before UNIX epoch".to_string())?
        .as_secs() as i64;
    Ok(format_unix_utc(seconds))
}

fn format_unix_utc(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    const REAL_Q8_MANIFEST: &str =
        include_str!("../../../examples/tiny-test-model-q8.minimodel.manifest.v0");

    #[test]
    fn formats_unix_epoch_utc() {
        assert_eq!(format_unix_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_utc(1_782_950_400), "2026-07-02T00:00:00Z");
    }

    #[test]
    fn verifies_real_q8_manifest_and_sibling_artifact_when_present() {
        let Some(path) = sibling_q8_fixture() else {
            eprintln!("skipping artifact verification test; TinyRustLM fixture not present");
            return;
        };
        let manifest = Manifest::parse(REAL_Q8_MANIFEST).unwrap();
        manifest.validate().unwrap();
        let artifact = std::fs::read(path).unwrap();
        let report =
            verify_artifact_bytes(&manifest, REAL_Q8_MANIFEST.as_bytes(), &artifact).unwrap();
        assert_eq!(report.artifact_byte_count, 8832);
        assert_eq!(
            report.artifact_sha256,
            "sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf"
        );
        assert_eq!(report.slm.quantization, "q8_0");
    }

    #[test]
    fn rejects_artifact_byte_count_drift_when_fixture_present() {
        let Some(path) = sibling_q8_fixture() else {
            eprintln!("skipping artifact drift test; TinyRustLM fixture not present");
            return;
        };
        let manifest = Manifest::parse(
            &REAL_Q8_MANIFEST.replace("artifact.byte_count=8832", "artifact.byte_count=8833"),
        )
        .unwrap();
        manifest.validate().unwrap();
        let artifact = std::fs::read(path).unwrap();
        let error =
            verify_artifact_bytes(&manifest, REAL_Q8_MANIFEST.as_bytes(), &artifact).unwrap_err();
        assert!(error.contains("byte count"));
        manifest
            .fields()
            .get("artifact.byte_count")
            .expect("manifest field used above");
    }

    #[test]
    fn rejects_artifact_sha256_drift_when_fixture_present() {
        let Some(path) = sibling_q8_fixture() else {
            eprintln!("skipping artifact SHA drift test; TinyRustLM fixture not present");
            return;
        };
        let manifest_text = REAL_Q8_MANIFEST.replace(
            "artifact.sha256=sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf",
            "artifact.sha256=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        );
        let manifest = Manifest::parse(&manifest_text).unwrap();
        manifest.validate().unwrap();
        let artifact = std::fs::read(path).unwrap();
        let error =
            verify_artifact_bytes(&manifest, manifest_text.as_bytes(), &artifact).unwrap_err();
        assert!(error.contains("SHA-256 mismatch"));
    }

    #[test]
    fn receipt_uses_basename_only() {
        let report = ArtifactVerification {
            manifest_sha256:
                "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
            manifest_model_id: "tinyrustlm/tiny-test-model-q8".to_string(),
            artifact_byte_count: 8832,
            artifact_sha256:
                "sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf"
                    .to_string(),
            evidence_status: "passed".to_string(),
            slm: SlmReport {
                file_bytes: 8832,
                format_version: 1,
                model_shape:
                    "vocab=260,hidden=8,layers=1,heads=2,kv_heads=2,head_dim=4,ffn=16,context=16"
                        .to_string(),
                quantization: "q8_0".to_string(),
                checksum: "0x481f1037d57ebcd3".to_string(),
                tokenizer_checksum: "0x288f65d94c26743e".to_string(),
                tensor_layout_checksum: "0xd8fb6e636a8badd6".to_string(),
            },
        };
        let receipt = import_receipt_text(
            &report,
            Path::new("C:/Users/AI/Downloads/tiny-test-model-q8.slm"),
        )
        .unwrap();
        assert!(receipt.contains("artifact.local_path_hint=tiny-test-model-q8.slm"));
        assert!(!receipt.contains("C:/Users/AI/Downloads"));
        assert!(receipt.contains("import.next_gate=user-selected-tinyrustlm-local-file-open"));
    }

    fn sibling_q8_fixture() -> Option<PathBuf> {
        let rust_root = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3)?;
        let path = rust_root
            .join("SLMComposer.com")
            .join("tinyrustlm")
            .join("models")
            .join("tiny-test-model-q8.slm");
        path.exists().then_some(path)
    }
}
