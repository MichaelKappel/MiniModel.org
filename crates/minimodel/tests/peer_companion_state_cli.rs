use std::process::Command;

mod common;

use common::{minimodel_bin, output_text, q8_manifest_text, sibling_q8_fixture, unique_dir};

#[test]
fn write_peer_companion_state_cli_writes_metadata_only_state_for_prepared_share() {
    let Some(artifact_path) = sibling_q8_fixture() else {
        eprintln!("skipping peer companion state CLI test; TinyRustLM fixture not present");
        return;
    };
    let dir = unique_dir("peer-companion-state-cli");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");

    let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
    let piece_dir = dir.join("tiny-test-model-q8.peer-pieces");
    let state_path = dir.join("tiny-test-model-q8.peer-companion-state.v0.json");
    std::fs::write(&manifest_path, q8_manifest_text()).expect("write q8 manifest");

    let manifest_path_text = manifest_path.to_string_lossy().to_string();
    let artifact_path_text = artifact_path.to_string_lossy().to_string();
    let piece_dir_text = piece_dir.to_string_lossy().to_string();
    let state_path_text = state_path.to_string_lossy().to_string();

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
    let export_text = output_text(&export_output);
    assert!(export_text.contains("MiniModel peer pieces exported"));
    assert!(export_text.contains("piece_count: 1"));
    assert!(piece_dir.join("minimodel.peer_piece_set.v0").exists());

    let state_output = Command::new(minimodel_bin())
        .args([
            "write-peer-companion-state",
            &manifest_path_text,
            &piece_dir_text,
            &state_path_text,
            "http://127.0.0.1:8123/peer-pieces",
        ])
        .output()
        .expect("run write-peer-companion-state");
    assert!(
        state_output.status.success(),
        "{}",
        output_text(&state_output)
    );
    let state_cli_text = output_text(&state_output);
    assert!(state_cli_text.contains("MiniModel peer companion state written"));
    assert!(state_cli_text.contains("model.id: tinyrustlm/tiny-test-model-q8"));
    assert!(state_cli_text.contains("peer_base_url: http://127.0.0.1:8123/peer-pieces/"));
    assert!(state_cli_text.contains("piece_count: 1"));

    let state = std::fs::read_to_string(&state_path).expect("read companion state");
    assert!(state.contains("\"state_kind\": \"minimodel.peer_companion_state\""));
    assert!(state.contains("\"share_status\": \"ready-to-serve\""));
    assert!(state.contains("\"transfer_protocol\": \"direct-http-piece-server-v0\""));
    assert!(state.contains("\"peer_base_url\": \"http://127.0.0.1:8123/peer-pieces/\""));
    assert!(state.contains("\"consent_status\": \"operator-reviewed-consenting-peer\""));
    assert!(state.contains("\"piece_set_path_hint\": \"minimodel.peer_piece_set.v0\""));
    assert!(state.contains("\"piece_count\": 1"));
    assert!(state.contains("\"peer_piece_bytes_in_state\": false"));
    assert!(state.contains("\"project_server_serves_model_bytes\": false"));
    assert!(state.contains("\"project_server_runs_model_bytes\": false"));
    assert!(state.contains("\"project_server_proxies_model_bytes\": false"));
    assert!(!state.contains("artifact_project_server_url"));
    assert!(!state.contains("\"peer_sources\""));

    let inspect_output = Command::new(minimodel_bin())
        .args([
            "inspect-peer-companion-state",
            &state_path_text,
            &manifest_path_text,
            &piece_dir_text,
        ])
        .output()
        .expect("run inspect-peer-companion-state");
    assert!(
        inspect_output.status.success(),
        "{}",
        output_text(&inspect_output)
    );
    let inspect_text = output_text(&inspect_output);
    assert!(inspect_text.contains("MiniModel peer companion state inspection: PASS"));
    assert!(inspect_text.contains("model.id: tinyrustlm/tiny-test-model-q8"));
    assert!(inspect_text.contains("peer_base_url: http://127.0.0.1:8123/peer-pieces/"));
    assert!(inspect_text.contains("state_route: minimodel.peer_companion_state.v0.json"));
    assert!(inspect_text.contains("piece_set.sha256: sha256:"));
    assert!(inspect_text.contains("piece_count: 1"));
    assert!(inspect_text.contains("artifact.byte_count: 8832"));
    assert!(inspect_text.contains("metadata-only peer companion state; no model bytes embedded"));

    let piece_count_drift_path =
        dir.join("tiny-test-model-q8.peer-companion-state-piece-drift.v0.json");
    std::fs::write(
        &piece_count_drift_path,
        state.replace("\"piece_count\": 1", "\"piece_count\": 2"),
    )
    .expect("write drifted piece-count state");
    let piece_count_drift_path_text = piece_count_drift_path.to_string_lossy().to_string();
    let drift_output = Command::new(minimodel_bin())
        .args([
            "inspect-peer-companion-state",
            &piece_count_drift_path_text,
            &manifest_path_text,
            &piece_dir_text,
        ])
        .output()
        .expect("run inspect-peer-companion-state drift");
    assert!(
        !drift_output.status.success(),
        "{}",
        output_text(&drift_output)
    );
    let drift_text = output_text(&drift_output);
    assert!(
        drift_text.contains("peer companion state pieces.piece_count mismatch"),
        "{drift_text}"
    );

    let project_server_drift_path =
        dir.join("tiny-test-model-q8.peer-companion-state-project-server-drift.v0.json");
    std::fs::write(
        &project_server_drift_path,
        state.replace(
            "\"project_server_serves_model_bytes\": false",
            "\"project_server_serves_model_bytes\": true",
        ),
    )
    .expect("write drifted project-server state");
    let project_server_drift_path_text = project_server_drift_path.to_string_lossy().to_string();
    let project_server_output = Command::new(minimodel_bin())
        .args([
            "inspect-peer-companion-state",
            &project_server_drift_path_text,
            &manifest_path_text,
            &piece_dir_text,
        ])
        .output()
        .expect("run inspect-peer-companion-state project server drift");
    assert!(
        !project_server_output.status.success(),
        "{}",
        output_text(&project_server_output)
    );
    let project_server_text = output_text(&project_server_output);
    assert!(
        project_server_text.contains("project_server_serves_model_bytes"),
        "{project_server_text}"
    );
    assert!(
        project_server_text.contains("false"),
        "{project_server_text}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_peer_companion_state_cli_rejects_project_and_huggingface_hosts_before_manifest_read() {
    let dir = unique_dir("peer-companion-state-cli-blocked-host");
    let _ = std::fs::remove_dir_all(&dir);
    let cases = [
        ("project-root-dot", "http://MiniModel.org./peer-pieces/"),
        (
            "project-subdomain",
            "http://catalog.minimodel.org/peer-pieces/",
        ),
        ("huggingface-root", "http://huggingface.co/peer-pieces/"),
        (
            "huggingface-subdomain",
            "http://cdn.huggingface.co/peer-pieces/",
        ),
    ];

    for (label, peer_url) in cases {
        let state_path = dir.join(format!("{label}.state.json"));
        let state_path_text = state_path.to_string_lossy().to_string();

        let output = Command::new(minimodel_bin())
            .args([
                "write-peer-companion-state",
                "missing-manifest",
                "missing-pieces",
                &state_path_text,
                peer_url,
            ])
            .output()
            .expect("run write-peer-companion-state");

        assert!(
            !output.status.success(),
            "{label}: {}",
            output_text(&output)
        );
        let text = output_text(&output);
        assert!(text.contains("not a peer"), "{label}: {text}");
        assert!(text.contains("never serve model bytes"), "{label}: {text}");
        assert!(!text.contains("failed to read manifest"), "{label}: {text}");
        assert!(!state_path.exists(), "{label} unexpectedly wrote state");
    }
}
