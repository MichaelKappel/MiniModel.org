mod common;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use common::{minimodel_bin, output_text, q8_manifest_text, unique_dir};
use ed25519_dalek::{Signer, SigningKey};
use minimodel_core::Manifest;
use sha2::{Digest, Sha256};
use std::process::Command;

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn signed_manifest_and_key_json() -> (String, String) {
    let public_key_route = "../keys/minimodel-development-key-v0.public-key.json";
    let placeholder_sha = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let unsigned = q8_manifest_text();
    let without_signature = unsigned
        .replace(
            "signature.kind=unsigned-draft",
            "signature.kind=external-signature-v0",
        )
        .replace(
            "signature.key_id=none",
            "signature.key_id=minimodel-development-key-v0",
        )
        .replace(
            "signature.public_key_route=none",
            &format!("signature.public_key_route={public_key_route}"),
        )
        .replace(
            "signature.payload_sha256=none",
            &format!("signature.payload_sha256={placeholder_sha}"),
        )
        .replace("signature.value=none", "signature.value=temporary");
    let payload_sha256 = Manifest::parse(&without_signature)
        .expect("signed manifest shell should parse")
        .canonical_signature_payload_sha256();
    let with_payload = without_signature.replace(placeholder_sha, &payload_sha256);
    let manifest = Manifest::parse(&with_payload).expect("signed manifest should parse");
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let signature = signing_key.sign(manifest.canonical_signature_payload().as_bytes());
    let signed_manifest = with_payload.replace(
        "signature.value=temporary",
        &format!(
            "signature.value=ed25519-base64:{}",
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
  "public_key_route": "{public_key_route}",
  "public_key_base64": "{}",
  "public_key_sha256": "{}",
  "usage": "minimodel-manifest-signing-v0",
  "created_utc": "2026-07-06T00:00:00Z",
  "revoked": false
}}"#,
        BASE64_STANDARD.encode(public_key_bytes),
        sha256_prefixed(&public_key_bytes)
    );
    (signed_manifest, key_json)
}

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

#[test]
fn verify_signed_manifest_accepts_ed25519_key_metadata() {
    let dir = unique_dir("signed-manifest-cli");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let manifest_path = dir.join("model.minimodel.manifest.v0");
    let key_path = dir.join("minimodel-development-key-v0.public-key.json");
    let (manifest_text, key_json) = signed_manifest_and_key_json();
    std::fs::write(&manifest_path, manifest_text).expect("write signed manifest");
    std::fs::write(&key_path, key_json).expect("write public key");

    let output = Command::new(minimodel_bin())
        .arg("verify-signed-manifest")
        .arg(&manifest_path)
        .arg(&key_path)
        .output()
        .expect("run verify-signed-manifest");

    assert!(output.status.success(), "{}", output_text(&output));
    let text = output_text(&output);
    assert!(
        text.contains("MiniModel signed manifest validation: PASS"),
        "{text}"
    );
    assert!(text.contains("signature.algorithm: ed25519"), "{text}");
    assert!(
        text.contains("signature.key_id: minimodel-development-key-v0"),
        "{text}"
    );
    assert!(
        text.contains(
            "signature.public_key_route: ../keys/minimodel-development-key-v0.public-key.json"
        ),
        "{text}"
    );
}

#[test]
fn verify_signed_manifest_rejects_signature_drift() {
    let dir = unique_dir("signed-manifest-drift-cli");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let manifest_path = dir.join("model.minimodel.manifest.v0");
    let key_path = dir.join("minimodel-development-key-v0.public-key.json");
    let (manifest_text, key_json) = signed_manifest_and_key_json();
    std::fs::write(
        &manifest_path,
        manifest_text.replace("model.version=0.1.0-fixture", "model.version=0.1.1-drift"),
    )
    .expect("write signed manifest drift");
    std::fs::write(&key_path, key_json).expect("write public key");

    let output = Command::new(minimodel_bin())
        .arg("verify-signed-manifest")
        .arg(&manifest_path)
        .arg(&key_path)
        .output()
        .expect("run verify-signed-manifest");

    assert!(!output.status.success(), "{}", output_text(&output));
    assert!(output_text(&output).contains("signature.payload_sha256 mismatch"));
}
