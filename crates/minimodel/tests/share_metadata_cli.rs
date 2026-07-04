use std::process::Command;

mod common;

use common::{minimodel_bin, output_text, unique_dir};

const ARTIFACT_SHA256: &str =
    "sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf";
const CHUNK_LIST_SHA256: &str =
    "sha256:022041f5926280b354db4b992c7ec6e2b1ff08c5dc087990da2c5dd965bbd76f";

#[test]
fn inspect_share_metadata_cli_accepts_metadata_only_bundle() {
    let dir = unique_dir("share-metadata-cli-metadata-only");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");
    let bundle_path = dir.join("metadata-only.minimodel-share-metadata.json");
    std::fs::write(&bundle_path, share_metadata_bundle(false)).expect("write bundle");
    let bundle_path_text = bundle_path.to_string_lossy().to_string();

    let output = Command::new(minimodel_bin())
        .args(["inspect-share-metadata", &bundle_path_text])
        .output()
        .expect("run inspect-share-metadata");

    assert!(output.status.success(), "{}", output_text(&output));
    let text = output_text(&output);
    assert!(text.contains("MiniModel share metadata validation: PASS"));
    assert!(text.contains("model.id: converted/example-org/tiny-source-model"));
    assert!(text.contains("artifact_file_name: tiny-test-model-q8.slm"));
    assert!(text.contains("artifact.byte_count: 8832"));
    assert!(text.contains(ARTIFACT_SHA256));
    assert!(text.contains("peer_transfer_status: metadata-only"));
    assert!(text.contains("share_kit_available: false"));
    assert!(text.contains("network_transfer_available: false"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inspect_share_metadata_cli_accepts_share_kit_ready_bundle() {
    let dir = unique_dir("share-metadata-cli-share-kit");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");
    let bundle_path = dir.join("share-kit.minimodel-share-metadata.json");
    std::fs::write(&bundle_path, share_metadata_bundle(true)).expect("write bundle");
    let bundle_path_text = bundle_path.to_string_lossy().to_string();

    let output = Command::new(minimodel_bin())
        .args(["inspect-share-metadata", &bundle_path_text])
        .output()
        .expect("run inspect-share-metadata");

    assert!(output.status.success(), "{}", output_text(&output));
    let text = output_text(&output);
    assert!(text.contains("MiniModel share metadata validation: PASS"));
    assert!(text.contains("peer_transfer_implementation: generated-local-minimodel-share-kit-v0"));
    assert!(text.contains("peer_transfer_status: local-share-kit-script-ready-known-peer-needed"));
    assert!(text.contains("share_kit_available: true"));
    assert!(text.contains("network_transfer_available: false"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inspect_share_metadata_cli_rejects_project_server_byte_claim() {
    let dir = unique_dir("share-metadata-cli-server-claim");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");
    let bundle_path = dir.join("unsafe.minimodel-share-metadata.json");
    let unsafe_bundle = share_metadata_bundle(true).replace(
        "\"project_server_serves_model_bytes\": false",
        "\"project_server_serves_model_bytes\": true",
    );
    std::fs::write(&bundle_path, unsafe_bundle).expect("write bundle");
    let bundle_path_text = bundle_path.to_string_lossy().to_string();

    let output = Command::new(minimodel_bin())
        .args(["inspect-share-metadata", &bundle_path_text])
        .output()
        .expect("run inspect-share-metadata");

    assert!(!output.status.success(), "{}", output_text(&output));
    assert!(output_text(&output).contains("project_server_serves_model_bytes must be false"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inspect_share_metadata_cli_rejects_chunk_list_drift() {
    let dir = unique_dir("share-metadata-cli-chunk-drift");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");
    let bundle_path = dir.join("drift.minimodel-share-metadata.json");
    let drifted_bundle = share_metadata_bundle(false).replace(
        CHUNK_LIST_SHA256,
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
    );
    std::fs::write(&bundle_path, drifted_bundle).expect("write bundle");
    let bundle_path_text = bundle_path.to_string_lossy().to_string();

    let output = Command::new(minimodel_bin())
        .args(["inspect-share-metadata", &bundle_path_text])
        .output()
        .expect("run inspect-share-metadata");

    assert!(!output.status.success(), "{}", output_text(&output));
    assert!(output_text(&output).contains("chunk_list_sha256"));
    assert!(output_text(&output).contains("recomputed"));

    let _ = std::fs::remove_dir_all(&dir);
}

fn share_metadata_bundle(share_kit_ready: bool) -> String {
    let boundary_profile = if share_kit_ready {
        r#"
      "peer_transfer_implemented": true,
      "peer_transfer_status": "local-share-kit-script-ready-known-peer-needed",
      "peer_transfer_implementation": "generated-local-minimodel-share-kit-v0""#
    } else {
        r#"
      "peer_transfer_implemented": false,
      "peer_transfer_status": "metadata-only",
      "peer_transfer_implementation": "not-included-in-metadata-export""#
    };
    let offer_status = if share_kit_ready {
        "local-share-kit-ready-known-peer-needed"
    } else {
        "metadata-ready-transfer-not-implemented"
    };
    let transfer_profile = if share_kit_ready {
        r#""protocols": [
          "minimodel-prepare-peer-share-v0",
          "known-direct-http-peer-v0",
          "minimodel-peer-companion-v0"
        ],
        "network_transfer_available": false,
        "local_share_kit_available": true,
        "serve_helper_available": true,
        "project_server_serves_model_bytes": false,
        "project_server_runs_model_bytes": false,
        "required_next_gate": "run-generated-share-kit-set-known-peer-url-and-serve-from-user-machine""#
    } else {
        r#""protocols": [],
        "network_transfer_available": false,
        "local_share_kit_available": false,
        "serve_helper_available": false,
        "project_server_serves_model_bytes": false,
        "project_server_runs_model_bytes": false,
        "required_next_gate": "implement-consent-based-peer-transfer-and-rust-reverify-before-import""#
    };
    let share_kit = if share_kit_ready {
        r#",
  "share_kit": {
    "kit_kind": "tinyrustlm.local_p2p_share_kit.v0",
    "kit_status": "script-ready-known-peer-needed",
    "prepares_peer_pieces": true,
    "writes_catalog_snapshot": true,
    "writes_peer_source_metadata_when_known_peer_set": true,
    "writes_peer_companion_state_when_known_peer_set": true,
    "writes_peer_contact_card_when_known_peer_set": true,
    "serve_helper_available": true,
    "user_consent_required_before_transfer": true,
    "project_server_serves_model_bytes": false,
    "project_server_runs_model_bytes": false,
    "execution_rule": "Run the generated PowerShell scripts locally after reviewing the verified .slm path and consenting peer URL."
  }"#
    } else {
        ""
    };

    format!(
        r#"{{
  "bundle_kind": "minimodel.local_slm_share_metadata.v0",
  "generated_at": "2026-07-04T12:00:00Z",
  "boundary": {{
    "project_server_runs_model_bytes": false,
    "project_server_serves_model_bytes": false,{boundary_profile},
    "user_consent_required_before_transfer": true,
    "tiny_rust_lm_execution_rule": "User imports a verified local .slm through the browser file picker before execution."
  }},
  "catalog_entry": {{
    "catalog_version": 0,
    "entry_kind": "minimodel.catalog_entry",
    "model_id": "converted/example-org/tiny-source-model",
    "display_name": "tiny-test-model-q8",
    "publisher_id": "local-user",
    "publisher_key_id": "local-user-unsigned",
    "artifact_kind": "slm",
    "artifact_file_name": "tiny-test-model-q8.slm",
    "local_file_name_hint": "tiny-test-model-q8.slm",
    "artifact_byte_count": 8832,
    "artifact_sha256": "{ARTIFACT_SHA256}",
    "artifact_project_server_url": null,
    "project_server_serves_model_bytes": false,
    "project_server_runs_model_bytes": false,
    "peer_hint_status": "metadata-ready-transfer-not-implemented",
    "execution_policy": "user-local-file-only",
    "source_kind": "huggingface-hub",
    "source_identity": "https://huggingface.co/example-org/tiny-source-model",
    "evidence_admission_status": "local-import-verified",
    "quality_claim": "not-endorsed-by-project",
    "trained_quality_claim": "not-claimed",
    "required_next_gate": "publisher-review-admission-and-peer-transfer-implementation",
    "slm_checksum": "0xa338bbb579e3e971",
    "source_huggingface_model_id": "example-org/tiny-source-model",
    "source_url": "https://huggingface.co/example-org/tiny-source-model"
  }},
  "peer_offer": {{
    "offer_kind": "minimodel.peer_offer.v0",
    "offer_version": 0,
    "offer_status": "{offer_status}",
    "artifact_kind": "slm",
    "artifact_file_name": "tiny-test-model-q8.slm",
    "artifact_byte_count": 8832,
    "artifact_sha256": "{ARTIFACT_SHA256}",
    "slm_checksum": "0xa338bbb579e3e971",
    "chunks": {{
      "algorithm": "sha256-merkle-v0",
      "chunk_size": 1048576,
      "chunk_count": 1,
      "chunk_list_sha256": "{CHUNK_LIST_SHA256}",
      "merkle_root_sha256": "{ARTIFACT_SHA256}",
      "entries": [
        {{
          "index": 0,
          "offset": 0,
          "byte_count": 8832,
          "sha256": "{ARTIFACT_SHA256}"
        }}
      ]
    }},
    "transfer": {{
      {transfer_profile}
    }}
  }},
  "conversion_request": {{
    "request_kind": "minimodel.huggingface_conversion_request.v0",
    "source": {{
      "kind": "huggingface_model",
      "model_id": "example-org/tiny-source-model",
      "model_url": "https://huggingface.co/example-org/tiny-source-model"
    }},
    "generated_at": "2026-07-02T00:00:00Z"
  }}{share_kit}
}}
"#
    )
}
