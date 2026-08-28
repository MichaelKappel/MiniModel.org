#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static UNIQUE_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn minimodel_bin() -> &'static str {
    env!("CARGO_BIN_EXE_minimodel")
}

pub fn unique_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after UNIX_EPOCH")
        .as_nanos();
    let sequence = UNIQUE_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "minimodel-{name}-{}-{nanos}-{sequence}",
        std::process::id()
    ))
}

pub fn output_text(output: &std::process::Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

pub fn q8_manifest_text() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-v0/tiny-test-model-q8.minimodel.manifest.v0"
    ))
}

pub fn q8_peer_import_request_text(peer_url: &str) -> String {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-v0/tiny-test-model-q8.minimodel-peer-import-request.json"
    ))
    .replace("http://127.0.0.1:8123/", peer_url)
}

pub fn sibling_q8_fixture() -> Option<PathBuf> {
    let rust_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)?;
    let path = rust_root
        .join("TinyRustLM.com")
        .join("tinyrustlm")
        .join("models")
        .join("tiny-test-model-q8.slm");
    path.exists().then_some(path)
}
