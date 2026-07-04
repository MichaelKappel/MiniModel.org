use std::process::Command;

mod common;

use common::{minimodel_bin, output_text, sibling_q8_fixture, unique_dir};

#[test]
fn prepare_peer_share_cli_writes_manifest_offer_pieces_and_catalog_entry() {
    let Some(artifact_path) = sibling_q8_fixture() else {
        eprintln!("skipping prepare peer share CLI test; TinyRustLM fixture not present");
        return;
    };
    let dir = unique_dir("prepare-peer-share-cli");
    let _ = std::fs::remove_dir_all(&dir);

    let output_dir = dir.join("share");
    let artifact_path_text = artifact_path.to_string_lossy().to_string();
    let output_dir_text = output_dir.to_string_lossy().to_string();

    let output = Command::new(minimodel_bin())
        .args([
            "prepare-peer-share",
            &artifact_path_text,
            &output_dir_text,
            "tinyrustlm/prepared-tiny-test-model-q8",
            "ExampleOrg/Tiny-Q8-Source",
            "http://127.0.0.1:8123/peer-pieces",
        ])
        .output()
        .expect("run prepare-peer-share");

    assert!(output.status.success(), "{}", output_text(&output));
    let cli_text = output_text(&output);
    assert!(cli_text.contains("MiniModel peer share prepared"));
    assert!(cli_text.contains("model.id: tinyrustlm/prepared-tiny-test-model-q8"));
    assert!(cli_text.contains("piece_count: 1"));
    assert!(cli_text.contains("peer_source_count: 1"));
    assert!(cli_text.contains("catalog_snapshot_hint: minimodel write-catalog-snapshot"));
    assert!(cli_text.contains("serve_hint: minimodel serve-peer-pieces"));

    let manifest_path = output_dir.join("tiny-test-model-q8.minimodel.manifest.v0");
    let peer_offer_path = output_dir.join("tiny-test-model-q8.minimodel-peer-offer");
    let piece_dir = output_dir.join("tiny-test-model-q8.peer-pieces");
    let catalog_entry_path = output_dir.join("tiny-test-model-q8.catalog-entry.v0.json");
    assert!(manifest_path.exists());
    assert!(peer_offer_path.exists());
    assert!(piece_dir.join("minimodel.peer_piece_set.v0").exists());
    assert!(catalog_entry_path.exists());

    let manifest = std::fs::read_to_string(manifest_path).expect("read prepared manifest");
    assert!(manifest.contains("model.id=tinyrustlm/prepared-tiny-test-model-q8"));
    assert!(manifest.contains("source.huggingface_model_id=ExampleOrg/Tiny-Q8-Source"));
    assert!(manifest.contains("artifact.acquisition=user-external-download"));
    assert!(manifest.contains("chunks.mode=fixed-size-merkle-v0"));
    assert!(!manifest.contains("artifact_project_server_url"));

    let peer_offer = std::fs::read_to_string(peer_offer_path).expect("read peer offer");
    assert!(peer_offer.contains("offer_kind=minimodel.peer_offer.v0"));
    assert!(peer_offer.contains("chunk_count=1"));
    assert!(!peer_offer.contains("artifact_project_server_url"));

    let catalog = std::fs::read_to_string(catalog_entry_path).expect("read catalog entry");
    assert!(catalog.contains("\"model_id\": \"tinyrustlm/prepared-tiny-test-model-q8\""));
    assert!(catalog.contains("\"source_huggingface_model_id\": \"ExampleOrg/Tiny-Q8-Source\""));
    assert!(catalog.contains("\"peer_source_count\": 1"));
    assert!(catalog.contains("\"peer_base_url\": \"http://127.0.0.1:8123/peer-pieces/\""));
    assert!(catalog.contains("\"source_kind\": \"known-direct-http-peer-v0\""));
    assert!(catalog.contains("\"transport\": \"direct-http-piece-server-v0\""));
    assert!(!catalog.contains("artifact_project_server_url\": \"http"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn prepare_peer_share_cli_rejects_project_and_huggingface_hosts_before_artifact_read() {
    let dir = unique_dir("prepare-peer-share-cli-blocked-host");
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
        let output_dir = dir.join(format!("share-{label}"));
        let output_dir_text = output_dir.to_string_lossy().to_string();

        let output = Command::new(minimodel_bin())
            .args([
                "prepare-peer-share",
                "missing.slm",
                &output_dir_text,
                "tinyrustlm/prepared-tiny-test-model-q8",
                peer_url,
            ])
            .output()
            .expect("run prepare-peer-share");

        assert!(
            !output.status.success(),
            "{label}: {}",
            output_text(&output)
        );
        let text = output_text(&output);
        assert!(text.contains("not a peer"), "{label}: {text}");
        assert!(text.contains("never serve model bytes"), "{label}: {text}");
        assert!(!output_dir.exists(), "{label} unexpectedly wrote output");
    }
}
