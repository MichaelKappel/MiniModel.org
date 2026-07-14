use serde::Serialize;
use tinyrustlm_slm::{inspect_slm_compatibility_bytes, SlmInspection};

#[derive(Debug, Clone, Serialize)]
pub struct SlmReport {
    pub file_bytes: usize,
    pub format_version: u32,
    pub model_shape: String,
    pub quantization: String,
    pub checksum: String,
    pub tokenizer_checksum: String,
    pub tensor_layout_checksum: String,
}

pub fn validate_slm_bytes(bytes: &[u8]) -> Result<SlmReport, String> {
    let identity = inspect_slm_compatibility_bytes(bytes)
        .map_err(|error| format!("TinyRustLM runtime rejected .slm artifact: {error}"))?;
    let inspection = &identity.inspection;
    Ok(SlmReport {
        file_bytes: bytes.len(),
        format_version: inspection.slm_version,
        model_shape: shape_summary(inspection),
        quantization: quantization_label(inspection).to_string(),
        checksum: format!("0x{:016x}", inspection.runtime_checksum),
        tokenizer_checksum: format!("0x{:016x}", identity.tokenizer_checksum),
        tensor_layout_checksum: format!("0x{:016x}", identity.tensor_layout_checksum),
    })
}

fn shape_summary(inspection: &SlmInspection) -> String {
    format!(
        "vocab={},hidden={},layers={},heads={},kv_heads={},head_dim={},ffn={},context={}",
        inspection.vocab_size,
        inspection.hidden_size,
        inspection.layer_count,
        inspection.head_count,
        inspection.kv_head_count,
        inspection.head_dim,
        inspection.ffn_size,
        inspection.max_context
    )
}

fn quantization_label(inspection: &SlmInspection) -> &'static str {
    let modes = [
        inspection.f32_tensor_count > 0,
        inspection.q8_0_tensor_count > 0,
        inspection.q5_0_tensor_count > 0,
        inspection.q4_0_tensor_count > 0,
    ];
    if modes.into_iter().filter(|present| *present).count() != 1 {
        return "mixed";
    }
    if inspection.f32_tensor_count > 0 {
        "f32"
    } else if inspection.q8_0_tensor_count > 0 {
        "q8_0"
    } else if inspection.q5_0_tensor_count > 0 {
        "q5_0"
    } else {
        "q4_0"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn validates_sibling_q8_fixture_with_stable_manifest_identity() {
        let bytes = fs::read(sibling_model("tiny-test-model-q8.slm")).unwrap();

        let report = validate_slm_bytes(&bytes).unwrap();

        assert_eq!(report.file_bytes, 8832);
        assert_eq!(
            report.model_shape,
            "vocab=260,hidden=8,layers=1,heads=2,kv_heads=2,head_dim=4,ffn=16,context=16"
        );
        assert_eq!(report.quantization, "q8_0");
        assert_eq!(report.checksum, "0x481f1037d57ebcd3");
        assert_eq!(report.tokenizer_checksum, "0x288f65d94c26743e");
        assert_eq!(report.tensor_layout_checksum, "0xd8fb6e636a8badd6");
    }

    #[test]
    fn validates_current_q5_runtime_format() {
        let bytes = fs::read(sibling_model("tiny-test-model-q5.slm")).unwrap();

        let report = validate_slm_bytes(&bytes).unwrap();

        assert_eq!(report.quantization, "q5_0");
    }

    #[test]
    fn validates_current_gpt2_bpe_runtime_format() {
        let bytes = fs::read(sibling_model("smollm2-135m-instruct-q8.slm")).unwrap();

        let report = validate_slm_bytes(&bytes).unwrap();

        assert_eq!(report.file_bytes, 136_863_168);
        assert_eq!(report.quantization, "q8_0");
        assert!(report.model_shape.contains("vocab=49152"));
    }

    fn sibling_model(file_name: &str) -> PathBuf {
        let rust_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("MiniModel repository must be under the shared Rust root");
        let path = rust_root
            .join("TinyRustLM.com")
            .join("tinyrustlm")
            .join("models")
            .join(file_name);
        assert!(
            path.is_file(),
            "missing TinyRustLM authority model: {}",
            path.display()
        );
        path
    }
}
