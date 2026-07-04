use std::process::Command;

mod common;

use common::{minimodel_bin, output_text, unique_dir};

#[test]
fn write_and_inspect_peer_contact_card_cli_round_trips_normalized_url() {
    let dir = unique_dir("peer-contact-card-cli");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");
    let card_path = dir.join("known-peer.card.json");
    let card_path_text = card_path.to_string_lossy().to_string();

    let write_output = Command::new(minimodel_bin())
        .args([
            "write-peer-contact-card",
            &card_path_text,
            "http://127.0.0.1:8123/peer-pieces",
            "Local CLI peer",
        ])
        .output()
        .expect("run write-peer-contact-card");
    assert!(
        write_output.status.success(),
        "{}",
        output_text(&write_output)
    );
    let write_text = output_text(&write_output);
    assert!(write_text.contains("MiniModel peer contact card written"));
    assert!(write_text.contains("peer_label: Local CLI peer"));
    assert!(write_text.contains("peer_base_url: http://127.0.0.1:8123/peer-pieces/"));

    let card = std::fs::read_to_string(&card_path).expect("read contact card");
    assert!(card.contains("\"card_kind\": \"minimodel.peer_contact_card\""));
    assert!(card.contains("\"peer_base_url\": \"http://127.0.0.1:8123/peer-pieces/\""));
    assert!(card.contains("\"metadata_only\": true"));
    assert!(card.contains("\"peer_contact_card_embeds_model_bytes\": false"));
    assert!(!card.contains("artifact_project_server_url"));
    assert!(!card.contains("\"peer_sources\""));

    let inspect_output = Command::new(minimodel_bin())
        .args(["inspect-peer-contact-card", &card_path_text])
        .output()
        .expect("run inspect-peer-contact-card");
    assert!(
        inspect_output.status.success(),
        "{}",
        output_text(&inspect_output)
    );
    let inspect_text = output_text(&inspect_output);
    assert!(inspect_text.contains("MiniModel peer contact card validation: PASS"));
    assert!(inspect_text.contains("peer_label: Local CLI peer"));
    assert!(inspect_text.contains("peer_base_url: http://127.0.0.1:8123/peer-pieces/"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_peer_contact_card_cli_rejects_project_and_huggingface_hosts_before_output() {
    let dir = unique_dir("peer-contact-card-cli-blocked-host");
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
        let card_path = dir.join(format!("{label}.card.json"));
        let card_path_text = card_path.to_string_lossy().to_string();

        let output = Command::new(minimodel_bin())
            .args([
                "write-peer-contact-card",
                &card_path_text,
                peer_url,
                "Blocked host",
            ])
            .output()
            .expect("run write-peer-contact-card");

        assert!(
            !output.status.success(),
            "{label}: {}",
            output_text(&output)
        );
        let text = output_text(&output);
        assert!(text.contains("not a peer"), "{label}: {text}");
        assert!(text.contains("never serve model bytes"), "{label}: {text}");
        assert!(!card_path.exists(), "{label} unexpectedly wrote a card");
    }
}
