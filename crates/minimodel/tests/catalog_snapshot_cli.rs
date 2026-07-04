use std::path::Path;
use std::process::Command;

mod common;

use common::{minimodel_bin, output_text, sibling_q8_fixture, unique_dir};

fn path_text(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[test]
fn inspect_catalog_snapshot_cli_validates_generated_snapshots() {
    let Some(artifact_path) = sibling_q8_fixture() else {
        eprintln!("skipping catalog snapshot CLI test; TinyRustLM fixture not present");
        return;
    };
    let dir = unique_dir("catalog-snapshot-cli");
    let _ = std::fs::remove_dir_all(&dir);

    let share_dir = dir.join("share");
    let proof_share_dir = dir.join("proof-share");
    let catalog_path = dir.join("models.v0.json");
    let feed_bound_catalog_path = dir.join("models.with-announcement-feed.v0.json");
    let feed_path = dir.join("peer-source-announcements.v0.json");
    let artifact_path_text = path_text(&artifact_path);
    let share_dir_text = path_text(&share_dir);
    let proof_share_dir_text = path_text(&proof_share_dir);

    let prepare_output = Command::new(minimodel_bin())
        .args([
            "prepare-peer-share",
            &artifact_path_text,
            &share_dir_text,
            "tinyrustlm/catalog-inspect-q8",
            "ExampleOrg/Tiny-Q8-Source",
            "http://127.0.0.1:8123/peer-pieces",
        ])
        .output()
        .expect("run prepare-peer-share");
    assert!(
        prepare_output.status.success(),
        "{}",
        output_text(&prepare_output)
    );

    let proof_prepare_output = Command::new(minimodel_bin())
        .args([
            "prepare-peer-share",
            &artifact_path_text,
            &proof_share_dir_text,
            "tinyrustlm/catalog-inspect-feed-q8",
            "ExampleOrg/Tiny-Q8-Source",
        ])
        .output()
        .expect("run proof-ready prepare-peer-share");
    assert!(
        proof_prepare_output.status.success(),
        "{}",
        output_text(&proof_prepare_output)
    );

    let catalog_entry_path = share_dir.join("tiny-test-model-q8.catalog-entry.v0.json");
    let proof_ready_entry_path = proof_share_dir.join("tiny-test-model-q8.catalog-entry.v0.json");
    let announcement_path = dir.join("tiny-test-model-q8.peer-source-announcement.v0.json");
    let catalog_entry_path_text = path_text(&catalog_entry_path);
    let proof_ready_entry_path_text = path_text(&proof_ready_entry_path);
    let announcement_path_text = path_text(&announcement_path);
    let feed_path_text = path_text(&feed_path);
    let catalog_path_text = path_text(&catalog_path);
    let feed_bound_catalog_path_text = path_text(&feed_bound_catalog_path);

    let write_catalog_output = Command::new(minimodel_bin())
        .args([
            "write-catalog-snapshot",
            &catalog_path_text,
            &catalog_entry_path_text,
        ])
        .output()
        .expect("run write-catalog-snapshot");
    assert!(
        write_catalog_output.status.success(),
        "{}",
        output_text(&write_catalog_output)
    );

    let inspect_catalog_output = Command::new(minimodel_bin())
        .args(["inspect-catalog-snapshot", &catalog_path_text])
        .output()
        .expect("run inspect-catalog-snapshot");
    assert!(
        inspect_catalog_output.status.success(),
        "{}",
        output_text(&inspect_catalog_output)
    );
    let inspect_text = output_text(&inspect_catalog_output);
    assert!(inspect_text.contains("MiniModel catalog snapshot validation: PASS"));
    assert!(inspect_text.contains("entry_count: 1"));
    assert!(inspect_text.contains("p2p_ready_count: 1"));
    assert!(inspect_text.contains("peer_source_count: 1"));
    assert!(inspect_text.contains("announcement_route_count: 0"));
    assert!(inspect_text.contains("catalog.sha256: sha256:"));

    let announcement_output = Command::new(minimodel_bin())
        .args([
            "write-peer-source-announcement",
            &proof_ready_entry_path_text,
            &announcement_path_text,
            "http://127.0.0.1:8123/peer-pieces",
        ])
        .output()
        .expect("run write-peer-source-announcement");
    assert!(
        announcement_output.status.success(),
        "{}",
        output_text(&announcement_output)
    );

    let write_feed_output = Command::new(minimodel_bin())
        .args([
            "write-peer-source-announcement-feed",
            &feed_path_text,
            &announcement_path_text,
        ])
        .output()
        .expect("run write-peer-source-announcement-feed");
    assert!(
        write_feed_output.status.success(),
        "{}",
        output_text(&write_feed_output)
    );

    let write_feed_catalog_output = Command::new(minimodel_bin())
        .args([
            "write-catalog-snapshot-with-announcement-feed",
            &feed_bound_catalog_path_text,
            "peer-source-announcements.v0.json",
            &feed_path_text,
            &proof_ready_entry_path_text,
        ])
        .output()
        .expect("run write-catalog-snapshot-with-announcement-feed");
    assert!(
        write_feed_catalog_output.status.success(),
        "{}",
        output_text(&write_feed_catalog_output)
    );

    let inspect_feed_catalog_output = Command::new(minimodel_bin())
        .args(["inspect-catalog-snapshot", &feed_bound_catalog_path_text])
        .output()
        .expect("run inspect-catalog-snapshot feed-bound");
    assert!(
        inspect_feed_catalog_output.status.success(),
        "{}",
        output_text(&inspect_feed_catalog_output)
    );
    let feed_inspect_text = output_text(&inspect_feed_catalog_output);
    assert!(feed_inspect_text.contains("entry_count: 1"));
    assert!(feed_inspect_text.contains("p2p_ready_count: 0"));
    assert!(feed_inspect_text.contains("peer_source_count: 0"));
    assert!(feed_inspect_text.contains("announcement_route_count: 1"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inspect_catalog_snapshot_cli_rejects_project_server_flag_drift() {
    let Some(artifact_path) = sibling_q8_fixture() else {
        eprintln!("skipping catalog snapshot CLI drift test; TinyRustLM fixture not present");
        return;
    };
    let dir = unique_dir("catalog-snapshot-cli-project-server-drift");
    let _ = std::fs::remove_dir_all(&dir);

    let share_dir = dir.join("share");
    let catalog_path = dir.join("models.v0.json");
    let artifact_path_text = path_text(&artifact_path);
    let share_dir_text = path_text(&share_dir);
    let catalog_path_text = path_text(&catalog_path);

    let prepare_output = Command::new(minimodel_bin())
        .args([
            "prepare-peer-share",
            &artifact_path_text,
            &share_dir_text,
            "tinyrustlm/catalog-inspect-drift-q8",
        ])
        .output()
        .expect("run prepare-peer-share");
    assert!(
        prepare_output.status.success(),
        "{}",
        output_text(&prepare_output)
    );

    let catalog_entry_path = share_dir.join("tiny-test-model-q8.catalog-entry.v0.json");
    let catalog_entry_path_text = path_text(&catalog_entry_path);
    let write_catalog_output = Command::new(minimodel_bin())
        .args([
            "write-catalog-snapshot",
            &catalog_path_text,
            &catalog_entry_path_text,
        ])
        .output()
        .expect("run write-catalog-snapshot");
    assert!(
        write_catalog_output.status.success(),
        "{}",
        output_text(&write_catalog_output)
    );

    let unsafe_catalog = std::fs::read_to_string(&catalog_path).unwrap().replacen(
        "\"project_server_serves_model_bytes\": false",
        "\"project_server_serves_model_bytes\": true",
        1,
    );
    std::fs::write(&catalog_path, unsafe_catalog).unwrap();

    let inspect_output = Command::new(minimodel_bin())
        .args(["inspect-catalog-snapshot", &catalog_path_text])
        .output()
        .expect("run inspect-catalog-snapshot");
    assert!(
        !inspect_output.status.success(),
        "{}",
        output_text(&inspect_output)
    );
    assert!(output_text(&inspect_output).contains("project_server_serves_model_bytes"));

    let _ = std::fs::remove_dir_all(&dir);
}
