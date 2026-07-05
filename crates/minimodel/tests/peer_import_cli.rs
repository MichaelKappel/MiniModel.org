use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

mod common;

use common::{
    minimodel_bin, output_text, q8_manifest_text, q8_peer_import_request_text, sibling_q8_fixture,
    unique_dir,
};

#[test]
fn inspect_peer_import_request_cli_validates_without_peer_server() {
    let dir = unique_dir("peer-import-inspect-cli");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");

    let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
    let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
    std::fs::write(&manifest_path, q8_manifest_text()).expect("write q8 manifest");
    std::fs::write(
        &request_path,
        q8_peer_import_request_text("http://127.0.0.1:9/peer-pieces/"),
    )
    .expect("write peer import request");

    let manifest_path_text = manifest_path.to_string_lossy().to_string();
    let request_path_text = request_path.to_string_lossy().to_string();
    let output = Command::new(minimodel_bin())
        .args([
            "inspect-peer-import-request",
            &request_path_text,
            &manifest_path_text,
        ])
        .output()
        .expect("run inspect-peer-import-request");
    assert!(output.status.success(), "{}", output_text(&output));
    let text = output_text(&output);
    assert!(text.contains("MiniModel peer import request validation: PASS"));
    assert!(text.contains("peer_base_url: http://127.0.0.1:9/peer-pieces/"));
    assert!(text.contains("metadata_routes.status: validated"));
    assert!(text.contains("metadata_routes.peer_status: minimodel.peer_status.v0.json"));
    assert!(text.contains("metadata_routes.peer_piece_set: minimodel.peer_piece_set.v0"));
    assert!(
        text.contains(
            "metadata_routes.peer_companion_state: minimodel.peer_companion_state.v0.json"
        ),
        "{text}"
    );
    assert!(text.contains("receiver_validation_contract.status: validated"));
    assert!(text.contains("receiver_validation.check_peer_status_before_piece_fetch: true"));
    assert!(text.contains("receiver_validation.preflight_fetches_piece_set_before_pieces: true"));
    assert!(
        text.contains(
            "receiver_validation.validate_advertised_companion_state_before_piece_fetch: true"
        ),
        "{text}"
    );
    assert!(text.contains("receiver_validation.import_receipt_required_before_browser_load: true"));
    assert!(text.contains("request.sha256: sha256:"));
    assert!(text.contains("manifest.sha256: sha256:"));
    assert!(text.contains("peer_piece_bytes_fetched: false"));
    assert!(text.contains("artifact_bytes_fetched: false"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inspect_peer_import_request_cli_rejects_project_and_huggingface_hosts_before_manifest_read() {
    let dir = unique_dir("peer-import-inspect-blocked-host-cli");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");

    let missing_manifest_path = dir.join("missing.minimodel.manifest.v0");
    let missing_manifest_path_text = missing_manifest_path.to_string_lossy().to_string();
    let cases = [
        ("project-root-dot", "http://MiniModel.org./peer-pieces/"),
        (
            "project-subdomain",
            "http://catalog.minimodel.org:8123/peer-pieces/",
        ),
        ("huggingface-root", "http://huggingface.co/peer-pieces/"),
        (
            "huggingface-subdomain",
            "http://cdn.huggingface.co:8123/peer-pieces/",
        ),
    ];

    for (label, peer_url) in cases {
        let request_path = dir.join(format!(
            "tiny-test-model-q8.{label}.minimodel-peer-import-request.json"
        ));
        std::fs::write(&request_path, q8_peer_import_request_text(peer_url))
            .expect("write peer import request");

        let request_path_text = request_path.to_string_lossy().to_string();
        let output = Command::new(minimodel_bin())
            .args([
                "inspect-peer-import-request",
                &request_path_text,
                &missing_manifest_path_text,
            ])
            .output()
            .expect("run inspect-peer-import-request");
        assert!(
            !output.status.success(),
            "{label}: {}",
            output_text(&output)
        );
        let text = output_text(&output);
        assert!(text.contains("not a peer"), "{label}: {text}");
        assert!(
            text.contains("project servers never serve model bytes"),
            "{label}: {text}"
        );
        assert!(
            text.contains("Hugging Face hosts never serve model bytes"),
            "{label}: {text}"
        );
        assert!(!text.contains("failed to read manifest"), "{label}: {text}");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inspect_peer_import_request_cli_rejects_unsafe_peer_path_before_manifest_read() {
    let dir = unique_dir("peer-import-inspect-unsafe-path-cli");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");

    let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
    let missing_manifest_path = dir.join("missing.minimodel.manifest.v0");
    std::fs::write(
        &request_path,
        q8_peer_import_request_text("http://127.0.0.1:9/peer-pieces/../secret/"),
    )
    .expect("write peer import request");

    let request_path_text = request_path.to_string_lossy().to_string();
    let missing_manifest_path_text = missing_manifest_path.to_string_lossy().to_string();
    let output = Command::new(minimodel_bin())
        .args([
            "inspect-peer-import-request",
            &request_path_text,
            &missing_manifest_path_text,
        ])
        .output()
        .expect("run inspect-peer-import-request");
    assert!(!output.status.success(), "{}", output_text(&output));
    let text = output_text(&output);
    assert!(text.contains("peer URL path must be a plain safe path"));
    assert!(!text.contains("failed to read manifest"));

    let _ = std::fs::remove_dir_all(&dir);
}

struct ChildGuard {
    child: Child,
}

impl ChildGuard {
    fn spawn(mut command: Command) -> Self {
        let child = command.spawn().expect("spawn serve-peer-companion");
        Self { child }
    }

    fn ensure_running(&mut self) {
        if let Some(status) = self.child.try_wait().expect("check peer server status") {
            panic!("serve-peer-companion exited early with {status}");
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn companion_peer_import_cli_executes_check_preflight_and_import_over_local_http() {
    let Some(artifact_path) = sibling_q8_fixture() else {
        eprintln!("skipping peer import CLI smoke; TinyRustLM fixture not present");
        return;
    };
    let dir = unique_dir("peer-import-cli");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");

    let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
    let piece_dir = dir.join("tiny-test-model-q8.peer-pieces");
    let state_path = dir.join("tiny-test-model-q8.peer-companion-state.v0.json");
    let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
    let status_path = dir.join("tiny-test-model-q8.peer-status-check.v0.json");
    let preflight_path = dir.join("tiny-test-model-q8.peer-import.preflight");
    let import_dir = dir.join("imported");
    std::fs::write(&manifest_path, q8_manifest_text()).expect("write q8 manifest");

    let bind_addr = free_loopback_addr();
    let peer_url = format!("http://{bind_addr}/peer-pieces/");
    std::fs::write(&request_path, q8_peer_import_request_text(&peer_url))
        .expect("write peer import request");

    let manifest_path_text = manifest_path.to_string_lossy().to_string();
    let artifact_path_text = artifact_path.to_string_lossy().to_string();
    let piece_dir_text = piece_dir.to_string_lossy().to_string();
    let state_path_text = state_path.to_string_lossy().to_string();
    let request_path_text = request_path.to_string_lossy().to_string();
    let status_path_text = status_path.to_string_lossy().to_string();
    let preflight_path_text = preflight_path.to_string_lossy().to_string();
    let import_dir_text = import_dir.to_string_lossy().to_string();

    let export_output = Command::new(minimodel_bin())
        .args([
            "export-peer-pieces",
            &manifest_path_text,
            &artifact_path_text,
            &piece_dir_text,
        ])
        .output()
        .expect("run export-peer-pieces");
    assert!(
        export_output.status.success(),
        "{}",
        output_text(&export_output)
    );

    let state_output = Command::new(minimodel_bin())
        .args([
            "write-peer-companion-state",
            &manifest_path_text,
            &piece_dir_text,
            &state_path_text,
            &peer_url,
        ])
        .output()
        .expect("run write-peer-companion-state");
    assert!(
        state_output.status.success(),
        "{}",
        output_text(&state_output)
    );

    let mut serve_command = Command::new(minimodel_bin());
    serve_command
        .args([
            "serve-peer-companion",
            &state_path_text,
            &manifest_path_text,
            &piece_dir_text,
            &bind_addr,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut server = ChildGuard::spawn(serve_command);

    let status_output_text = wait_for_peer_status(
        &mut server,
        &manifest_path_text,
        &peer_url,
        &status_path_text,
    );
    assert!(status_output_text.contains("MiniModel peer status check: PASS"));
    assert!(status_output_text.contains("peer_piece_bytes_fetched: false"));
    assert!(status_output_text.contains("artifact_bytes_fetched: false"));
    let status_report = std::fs::read_to_string(&status_path).expect("read peer status report");
    assert!(status_report.contains("\"check_kind\": \"minimodel.peer_status_check\""));
    assert!(status_report.contains("\"status\": \"validated\""));
    assert!(status_report.contains("\"peer_base_url\": \""));
    assert!(status_report.contains("/peer-pieces/"));
    assert!(status_report.contains("\"peer_piece_bytes_fetched\": false"));
    assert!(status_report.contains("\"artifact_bytes_fetched\": false"));

    let preflight_output = Command::new(minimodel_bin())
        .args([
            "preflight-peer-import-request",
            &request_path_text,
            &manifest_path_text,
            &preflight_path_text,
        ])
        .output()
        .expect("run preflight-peer-import-request");
    assert!(
        preflight_output.status.success(),
        "{}",
        output_text(&preflight_output)
    );
    let preflight_text = std::fs::read_to_string(&preflight_path).expect("read preflight");
    assert!(preflight_text.contains("preflight.kind=minimodel.peer_import_preflight"));
    assert!(preflight_text.contains("metadata_routes.status=validated"));
    assert!(preflight_text.contains("metadata_routes.peer_status=minimodel.peer_status.v0.json"));
    assert!(preflight_text.contains("metadata_routes.peer_piece_set=minimodel.peer_piece_set.v0"));
    assert!(preflight_text
        .contains("metadata_routes.peer_companion_state=minimodel.peer_companion_state.v0.json"));
    assert!(preflight_text.contains("receiver_validation_contract.status=validated"));
    assert!(
        preflight_text.contains("receiver_validation.check_peer_status_before_piece_fetch=true")
    );
    assert!(preflight_text
        .contains("receiver_validation.preflight_fetches_piece_set_before_pieces=true"));
    assert!(preflight_text.contains(
        "receiver_validation.validate_advertised_companion_state_before_piece_fetch=true"
    ));
    assert!(preflight_text
        .contains("receiver_validation.import_receipt_required_before_browser_load=true"));
    assert!(preflight_text.contains("peer.companion_state_status=validated"));
    assert!(preflight_text.contains("peer_piece_set_metadata_fetched=true"));
    assert!(preflight_text.contains("peer_companion_state_metadata_fetched=true"));
    assert!(preflight_text.contains("peer_piece_bytes_fetched=false"));
    assert!(preflight_text.contains("artifact_bytes_fetched=false"));
    assert!(preflight_text.contains("project_server_serves_model_bytes=false"));

    let inspect_preflight_output = Command::new(minimodel_bin())
        .args([
            "inspect-peer-import-preflight",
            &preflight_path_text,
            &request_path_text,
            &manifest_path_text,
        ])
        .output()
        .expect("run inspect-peer-import-preflight");
    assert!(
        inspect_preflight_output.status.success(),
        "{}",
        output_text(&inspect_preflight_output)
    );
    let inspect_preflight_text = output_text(&inspect_preflight_output);
    assert!(inspect_preflight_text.contains("MiniModel peer import preflight validation: PASS"));
    assert!(inspect_preflight_text.contains("metadata_routes.status: validated"));
    assert!(inspect_preflight_text.contains("receiver_validation_contract.status: validated"));
    assert!(inspect_preflight_text.contains("peer_piece_bytes_fetched: false"));
    assert!(inspect_preflight_text.contains("artifact_bytes_fetched: false"));
    assert!(inspect_preflight_text.contains("preflight.sha256: sha256:"));

    let drift_preflight_path = dir.join("tiny-test-model-q8.drift.peer-import.preflight");
    let drift_preflight_path_text = drift_preflight_path.to_string_lossy().to_string();
    std::fs::write(
        &drift_preflight_path,
        preflight_text.replace(
            "project_server_serves_model_bytes=false",
            "project_server_serves_model_bytes=true",
        ),
    )
    .expect("write drifted preflight");
    let drift_preflight_output = Command::new(minimodel_bin())
        .args([
            "inspect-peer-import-preflight",
            &drift_preflight_path_text,
            &request_path_text,
            &manifest_path_text,
        ])
        .output()
        .expect("run inspect-peer-import-preflight on drifted preflight");
    assert!(
        !drift_preflight_output.status.success(),
        "{}",
        output_text(&drift_preflight_output)
    );
    let drift_preflight_text = output_text(&drift_preflight_output);
    assert!(drift_preflight_text.contains("project_server_serves_model_bytes mismatch"));

    let import_output = Command::new(minimodel_bin())
        .args([
            "import-peer-request",
            &request_path_text,
            &manifest_path_text,
            &import_dir_text,
        ])
        .output()
        .expect("run import-peer-request");
    assert!(
        import_output.status.success(),
        "{}",
        output_text(&import_output)
    );
    let import_cli_text = output_text(&import_output);
    assert!(import_cli_text.contains("MiniModel peer import request executed"));
    assert!(import_cli_text.contains("companion_state_status: validated"));
    assert!(import_cli_text.contains("piece_count: 1"));

    let output_artifact_path = import_dir.join("tiny-test-model-q8.slm");
    let receipt_path = import_dir.join("tiny-test-model-q8.minimodel-receipt");
    let output_artifact_path_text = output_artifact_path.to_string_lossy().to_string();
    let receipt_path_text = receipt_path.to_string_lossy().to_string();
    assert_eq!(
        std::fs::read(&output_artifact_path).expect("read imported artifact"),
        std::fs::read(&artifact_path).expect("read source artifact")
    );
    let receipt = std::fs::read_to_string(&receipt_path).expect("read peer import receipt");
    assert!(receipt.contains("slm.validation_status=passed"));
    assert!(receipt.contains("peer_import.transport=known-direct-http-peer-v0"));
    assert!(receipt.contains("peer.companion_state_status=validated"));
    assert!(receipt.contains("peer_companion_state_metadata_fetched=true"));
    assert!(receipt.contains("peer_piece_bytes_fetched=true"));
    assert!(receipt.contains("artifact.assembled_from_peer_pieces=true"));
    assert!(receipt.contains("project_server_serves_model_bytes=false"));
    assert!(receipt.contains("receiver_validation_contract.status=validated"));
    assert!(receipt.contains("receiver_validation.check_peer_status_before_piece_fetch=true"));
    assert!(receipt.contains("receiver_validation.preflight_fetches_piece_set_before_pieces=true"));
    assert!(receipt.contains(
        "receiver_validation.validate_advertised_companion_state_before_piece_fetch=true"
    ));
    assert!(
        receipt.contains("receiver_validation.import_receipt_required_before_browser_load=true")
    );

    let inspect_receipt_output = Command::new(minimodel_bin())
        .args([
            "inspect-import-receipt",
            &receipt_path_text,
            &manifest_path_text,
            &output_artifact_path_text,
        ])
        .output()
        .expect("run inspect-import-receipt");
    assert!(
        inspect_receipt_output.status.success(),
        "{}",
        output_text(&inspect_receipt_output)
    );
    let inspect_receipt_text = output_text(&inspect_receipt_output);
    assert!(inspect_receipt_text.contains("MiniModel import receipt validation: PASS"));
    assert!(inspect_receipt_text.contains("peer_import: true"));
    assert!(inspect_receipt_text.contains("companion_state_status: validated"));
    assert!(inspect_receipt_text.contains("peer_piece_bytes_fetched: true"));
    assert!(inspect_receipt_text.contains("artifact.assembled_from_peer_pieces: true"));
    assert_receiver_validation_report_text(&inspect_receipt_text);

    let inspect_peer_receipt_output = Command::new(minimodel_bin())
        .args([
            "inspect-peer-import-receipt",
            &receipt_path_text,
            &request_path_text,
            &preflight_path_text,
            &manifest_path_text,
            &output_artifact_path_text,
        ])
        .output()
        .expect("run inspect-peer-import-receipt");
    assert!(
        inspect_peer_receipt_output.status.success(),
        "{}",
        output_text(&inspect_peer_receipt_output)
    );
    let inspect_peer_receipt_text = output_text(&inspect_peer_receipt_output);
    assert!(
        inspect_peer_receipt_text.contains("MiniModel peer import receipt chain validation: PASS")
    );
    assert!(inspect_peer_receipt_text.contains("companion_state_status: validated"));
    assert!(inspect_peer_receipt_text.contains("peer_piece_bytes_fetched: true"));
    assert!(inspect_peer_receipt_text.contains("preflight.sha256: sha256:"));
    assert_receiver_validation_report_text(&inspect_peer_receipt_text);

    let piece_set_sha_line = receipt
        .lines()
        .find(|line| line.starts_with("peer.piece_set_sha256="))
        .expect("receipt includes peer piece-set SHA");
    let drift_piece_set_receipt_path =
        import_dir.join("tiny-test-model-q8.piece-set-drift.minimodel-receipt");
    let drift_piece_set_receipt_path_text =
        drift_piece_set_receipt_path.to_string_lossy().to_string();
    std::fs::write(
        &drift_piece_set_receipt_path,
        receipt.replace(
            piece_set_sha_line,
            "peer.piece_set_sha256=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ),
    )
    .expect("write piece-set drifted receipt");
    let drift_piece_set_output = Command::new(minimodel_bin())
        .args([
            "inspect-peer-import-receipt",
            &drift_piece_set_receipt_path_text,
            &request_path_text,
            &preflight_path_text,
            &manifest_path_text,
            &output_artifact_path_text,
        ])
        .output()
        .expect("run inspect-peer-import-receipt on piece-set drifted receipt");
    assert!(
        !drift_piece_set_output.status.success(),
        "{}",
        output_text(&drift_piece_set_output)
    );
    assert!(output_text(&drift_piece_set_output).contains("peer.piece_set_sha256"));

    let receiver_validation_drift_receipt_path =
        import_dir.join("tiny-test-model-q8.receiver-validation-drift.minimodel-receipt");
    let receiver_validation_drift_receipt_path_text = receiver_validation_drift_receipt_path
        .to_string_lossy()
        .to_string();
    std::fs::write(
        &receiver_validation_drift_receipt_path,
        receipt.replace(
            "receiver_validation.import_receipt_required_before_browser_load=true",
            "receiver_validation.import_receipt_required_before_browser_load=false",
        ),
    )
    .expect("write receiver-validation drifted receipt");
    let receiver_validation_drift_output = Command::new(minimodel_bin())
        .args([
            "inspect-import-receipt",
            &receiver_validation_drift_receipt_path_text,
            &manifest_path_text,
            &output_artifact_path_text,
        ])
        .output()
        .expect("run inspect-import-receipt on receiver-validation drifted receipt");
    assert!(
        !receiver_validation_drift_output.status.success(),
        "{}",
        output_text(&receiver_validation_drift_output)
    );
    assert!(output_text(&receiver_validation_drift_output)
        .contains("receiver_validation.import_receipt_required_before_browser_load"));

    let receiver_validation_chain_drift_output = Command::new(minimodel_bin())
        .args([
            "inspect-peer-import-receipt",
            &receiver_validation_drift_receipt_path_text,
            &request_path_text,
            &preflight_path_text,
            &manifest_path_text,
            &output_artifact_path_text,
        ])
        .output()
        .expect("run inspect-peer-import-receipt on receiver-validation drifted receipt");
    assert!(
        !receiver_validation_chain_drift_output.status.success(),
        "{}",
        output_text(&receiver_validation_chain_drift_output)
    );
    assert!(output_text(&receiver_validation_chain_drift_output)
        .contains("receiver_validation.import_receipt_required_before_browser_load"));

    let future_preflight_path = dir.join("tiny-test-model-q8.future.peer-import.preflight");
    let future_preflight_path_text = future_preflight_path.to_string_lossy().to_string();
    let preflight_created_line = preflight_text
        .lines()
        .find(|line| line.starts_with("preflight.created_utc="))
        .expect("preflight includes creation timestamp");
    std::fs::write(
        &future_preflight_path,
        preflight_text.replace(
            preflight_created_line,
            "preflight.created_utc=9999-01-01T00:00:00Z",
        ),
    )
    .expect("write future-dated preflight");
    let future_preflight_output = Command::new(minimodel_bin())
        .args([
            "inspect-peer-import-receipt",
            &receipt_path_text,
            &request_path_text,
            &future_preflight_path_text,
            &manifest_path_text,
            &output_artifact_path_text,
        ])
        .output()
        .expect("run inspect-peer-import-receipt on future-dated preflight");
    assert!(
        !future_preflight_output.status.success(),
        "{}",
        output_text(&future_preflight_output)
    );
    let future_preflight_text = output_text(&future_preflight_output);
    assert!(future_preflight_text.contains("receipt.created_utc"));
    assert!(future_preflight_text.contains("preflight.created_utc"));

    let drift_receipt_path = import_dir.join("tiny-test-model-q8.drift.minimodel-receipt");
    let drift_receipt_path_text = drift_receipt_path.to_string_lossy().to_string();
    std::fs::write(
        &drift_receipt_path,
        receipt.replace(
            "project_server_serves_model_bytes=false",
            "project_server_serves_model_bytes=true",
        ),
    )
    .expect("write drifted receipt");
    let drift_output = Command::new(minimodel_bin())
        .args([
            "inspect-import-receipt",
            &drift_receipt_path_text,
            &manifest_path_text,
            &output_artifact_path_text,
        ])
        .output()
        .expect("run inspect-import-receipt on drifted receipt");
    assert!(
        !drift_output.status.success(),
        "{}",
        output_text(&drift_output)
    );
    assert!(output_text(&drift_output).contains("project_server_serves_model_bytes"));

    let _ = std::fs::remove_dir_all(&dir);
}

fn free_loopback_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral loopback port");
    listener
        .local_addr()
        .expect("read loopback address")
        .to_string()
}

fn wait_for_peer_status(
    server: &mut ChildGuard,
    manifest_path: &str,
    peer_url: &str,
    status_path: &str,
) -> String {
    let mut last_output = String::new();
    for _ in 0..50 {
        server.ensure_running();
        let output = Command::new(minimodel_bin())
            .args(["check-peer-status", manifest_path, peer_url, status_path])
            .output()
            .expect("run check-peer-status");
        last_output = output_text(&output);
        if output.status.success() {
            return last_output;
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("peer status never became ready:\n{last_output}");
}

fn assert_receiver_validation_report_text(text: &str) {
    assert!(text.contains("receiver_validation_contract.status: validated"));
    assert!(text.contains("receiver_validation.check_peer_status_before_piece_fetch: true"));
    assert!(text.contains("receiver_validation.preflight_fetches_piece_set_before_pieces: true"));
    assert!(text.contains(
        "receiver_validation.validate_advertised_companion_state_before_piece_fetch: true"
    ));
    assert!(text.contains("receiver_validation.import_receipt_required_before_browser_load: true"));
}
