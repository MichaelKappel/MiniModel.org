mod common;

use common::{minimodel_bin, output_text, q8_manifest_text, unique_dir};
use minimodel_core::Manifest;
use std::process::Command;

#[test]
fn verify_manifest_prints_canonical_signature_payload_digest() {
    let dir = unique_dir("manifest-cli");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let manifest_path = dir.join("model.minimodel.manifest.v0");
    std::fs::write(&manifest_path, q8_manifest_text()).expect("write q8 manifest");

    let manifest =
        Manifest::parse(q8_manifest_text()).expect("fixture manifest should parse successfully");
    let expected_digest = manifest.canonical_signature_payload_sha256();

    let output = Command::new(minimodel_bin())
        .arg("verify-manifest")
        .arg(&manifest_path)
        .output()
        .expect("run verify-manifest");

    assert!(output.status.success(), "{}", output_text(&output));
    let text = output_text(&output);
    assert!(
        text.contains("MiniModel manifest validation: PASS"),
        "{text}"
    );
    assert!(
        text.contains(&format!("signature.payload_sha256: {expected_digest}")),
        "{text}"
    );
}
