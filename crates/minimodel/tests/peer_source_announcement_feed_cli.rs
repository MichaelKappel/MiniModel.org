use std::path::Path;
use std::process::Command;

mod common;

use common::{minimodel_bin, output_text, sibling_q8_fixture, unique_dir};

fn path_text(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[test]
fn inspect_peer_source_announcement_feed_cli_round_trips_prepared_announcement() {
    let Some(artifact_path) = sibling_q8_fixture() else {
        eprintln!(
            "skipping peer source announcement feed CLI test; TinyRustLM fixture not present"
        );
        return;
    };
    let dir = unique_dir("peer-source-announcement-feed-cli");
    let _ = std::fs::remove_dir_all(&dir);

    let share_dir = dir.join("share");
    let artifact_path_text = path_text(&artifact_path);
    let share_dir_text = path_text(&share_dir);

    let prepare_output = Command::new(minimodel_bin())
        .args([
            "prepare-peer-share",
            &artifact_path_text,
            &share_dir_text,
            "tinyrustlm/feed-announcement-q8",
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

    let catalog_entry_path = share_dir.join("tiny-test-model-q8.catalog-entry.v0.json");
    let announcement_path = dir.join("tiny-test-model-q8.peer-source-announcement.v0.json");
    let feed_path = dir.join("peer-source-announcements.v0.json");
    let catalog_entry_path_text = path_text(&catalog_entry_path);
    let announcement_path_text = path_text(&announcement_path);
    let feed_path_text = path_text(&feed_path);

    let announcement_output = Command::new(minimodel_bin())
        .args([
            "write-peer-source-announcement",
            &catalog_entry_path_text,
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
    let write_text = output_text(&write_feed_output);
    assert!(write_text.contains("MiniModel peer source announcement feed written"));
    assert!(write_text.contains("announcement_count: 1"));
    assert!(write_text.contains("model_count: 1"));
    assert!(write_text.contains("feed.bytes: "));
    assert!(write_text.contains("feed.sha256: sha256:"));

    let feed = std::fs::read_to_string(&feed_path).expect("read announcement feed");
    assert!(feed
        .contains("\"announcement_catalog_kind\": \"minimodel.peer_source_announcement_catalog\""));
    assert!(feed.contains("\"project_server_serves_model_bytes\": false"));
    assert!(feed.contains("\"project_server_runs_model_bytes\": false"));
    assert!(feed.contains("\"project_server_proxies_model_bytes\": false"));
    assert!(feed.contains("\"source_kind\": \"known-direct-http-peer-v0\""));
    assert!(!feed.contains("artifact_project_server_url"));

    let inspect_output = Command::new(minimodel_bin())
        .args(["inspect-peer-source-announcement-feed", &feed_path_text])
        .output()
        .expect("run inspect-peer-source-announcement-feed");
    assert!(
        inspect_output.status.success(),
        "{}",
        output_text(&inspect_output)
    );
    let inspect_text = output_text(&inspect_output);
    assert!(inspect_text.contains("MiniModel peer source announcement feed validation: PASS"));
    assert!(inspect_text.contains("announcement_count: 1"));
    assert!(inspect_text.contains("model_count: 1"));
    assert!(inspect_text.contains("feed.bytes: "));
    assert!(inspect_text.contains("feed.sha256: sha256:"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inspect_peer_source_announcement_feed_cli_accepts_empty_feed() {
    let dir = unique_dir("peer-source-announcement-empty-feed-cli");
    let _ = std::fs::remove_dir_all(&dir);
    let feed_path = dir.join("peer-source-announcements.empty.v0.json");
    let feed_path_text = path_text(&feed_path);

    let write_feed_output = Command::new(minimodel_bin())
        .args(["write-peer-source-announcement-feed", &feed_path_text])
        .output()
        .expect("run write-peer-source-announcement-feed");
    assert!(
        write_feed_output.status.success(),
        "{}",
        output_text(&write_feed_output)
    );

    let inspect_output = Command::new(minimodel_bin())
        .args(["inspect-peer-source-announcement-feed", &feed_path_text])
        .output()
        .expect("run inspect-peer-source-announcement-feed");
    assert!(
        inspect_output.status.success(),
        "{}",
        output_text(&inspect_output)
    );
    let inspect_text = output_text(&inspect_output);
    assert!(inspect_text.contains("MiniModel peer source announcement feed validation: PASS"));
    assert!(inspect_text.contains("announcement_count: 0"));
    assert!(inspect_text.contains("model_count: 0"));
    assert!(inspect_text.contains("feed.bytes: "));
    assert!(inspect_text.contains("feed.sha256: sha256:"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inspect_peer_source_announcement_feed_cli_rejects_oversized_feed_before_parse() {
    let dir = unique_dir("peer-source-announcement-feed-byte-budget-cli");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");
    let feed_path = dir.join("peer-source-announcements.too-large.v0.json");
    let feed_path_text = path_text(&feed_path);
    std::fs::write(&feed_path, vec![b' '; (4 * 1024 * 1024) + 1])
        .expect("write oversized announcement feed");

    let inspect_output = Command::new(minimodel_bin())
        .args(["inspect-peer-source-announcement-feed", &feed_path_text])
        .output()
        .expect("run inspect-peer-source-announcement-feed");
    assert!(
        !inspect_output.status.success(),
        "{}",
        output_text(&inspect_output)
    );
    assert!(output_text(&inspect_output).contains("exceeded 4 MiB byte budget"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inspect_peer_source_announcement_feed_cli_rejects_project_server_flags() {
    let dir = unique_dir("peer-source-announcement-feed-project-server-cli");
    let _ = std::fs::remove_dir_all(&dir);
    let feed_path = dir.join("peer-source-announcements.v0.json");
    let feed_path_text = path_text(&feed_path);

    let write_feed_output = Command::new(minimodel_bin())
        .args(["write-peer-source-announcement-feed", &feed_path_text])
        .output()
        .expect("run write-peer-source-announcement-feed");
    assert!(
        write_feed_output.status.success(),
        "{}",
        output_text(&write_feed_output)
    );

    let feed = std::fs::read_to_string(&feed_path).expect("read announcement feed");
    let unsafe_feed = feed.replacen(
        "\"project_server_serves_model_bytes\": false",
        "\"project_server_serves_model_bytes\": true",
        1,
    );
    assert_ne!(feed, unsafe_feed);
    std::fs::write(&feed_path, unsafe_feed).expect("write unsafe announcement feed");

    let inspect_output = Command::new(minimodel_bin())
        .args(["inspect-peer-source-announcement-feed", &feed_path_text])
        .output()
        .expect("run inspect-peer-source-announcement-feed");
    assert!(
        !inspect_output.status.success(),
        "{}",
        output_text(&inspect_output)
    );
    assert!(output_text(&inspect_output).contains("project_server_serves_model_bytes"));

    let _ = std::fs::remove_dir_all(&dir);
}
