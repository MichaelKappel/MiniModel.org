use std::process::Command;

mod common;

use common::{minimodel_bin, output_text, unique_dir};

#[test]
fn write_and_inspect_peer_contact_book_cli_embeds_reviewed_cards() {
    let dir = unique_dir("peer-contact-book-cli");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");
    let first_card_path = dir.join("local-peer.card.json");
    let second_card_path = dir.join("office-peer.card.json");
    let book_path = dir.join("reviewed-peers.contact-book.json");
    let first_card_path_text = first_card_path.to_string_lossy().to_string();
    let second_card_path_text = second_card_path.to_string_lossy().to_string();
    let book_path_text = book_path.to_string_lossy().to_string();

    for (card_path, peer_url, label) in [
        (
            &first_card_path_text,
            "http://127.0.0.1:8123/peer-pieces",
            "Local CLI peer",
        ),
        (
            &second_card_path_text,
            "http://192.0.2.42:8123/peer-pieces",
            "Office CLI peer",
        ),
    ] {
        let output = Command::new(minimodel_bin())
            .args(["write-peer-contact-card", card_path, peer_url, label])
            .output()
            .expect("run write-peer-contact-card");
        assert!(output.status.success(), "{}", output_text(&output));
    }

    let write_output = Command::new(minimodel_bin())
        .args([
            "write-peer-contact-book",
            &book_path_text,
            &first_card_path_text,
            &second_card_path_text,
        ])
        .output()
        .expect("run write-peer-contact-book");
    assert!(
        write_output.status.success(),
        "{}",
        output_text(&write_output)
    );
    let write_text = output_text(&write_output);
    assert!(write_text.contains("MiniModel peer contact book written"));
    assert!(write_text.contains("contact_card_count: 2"));
    assert!(write_text.contains("first_peer_label: Local CLI peer"));
    assert!(write_text.contains("first_peer_base_url: http://127.0.0.1:8123/peer-pieces/"));

    let book = std::fs::read_to_string(&book_path).expect("read contact book");
    assert!(book.contains("\"book_kind\": \"minimodel.peer_contact_book.v0\""));
    assert!(book.contains("\"contact_card_count\": 2"));
    assert!(book.contains("\"card_sha256\": \"sha256:"));
    assert!(book.contains("\"peer_base_url\": \"http://127.0.0.1:8123/peer-pieces/\""));
    assert!(book.contains("\"peer_base_url\": \"http://192.0.2.42:8123/peer-pieces/\""));
    assert!(book.contains("\"peer_contact_book_embeds_model_bytes\": false"));
    assert!(book.contains("\"peer_contact_book_embeds_peer_piece_bytes\": false"));
    assert!(!book.contains(&first_card_path_text));
    assert!(!book.contains("artifact_project_server_url"));
    assert!(!book.contains("\"peer_sources\""));
    assert!(!book.contains(".slm"));

    let inspect_output = Command::new(minimodel_bin())
        .args(["inspect-peer-contact-book", &book_path_text])
        .output()
        .expect("run inspect-peer-contact-book");
    assert!(
        inspect_output.status.success(),
        "{}",
        output_text(&inspect_output)
    );
    let inspect_text = output_text(&inspect_output);
    assert!(inspect_text.contains("MiniModel peer contact book validation: PASS"));
    assert!(inspect_text.contains("contact_card_count: 2"));
    assert!(inspect_text.contains("boundary: metadata-only peer contact book"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_peer_contact_book_cli_rejects_duplicate_peer_urls_before_output() {
    let dir = unique_dir("peer-contact-book-cli-duplicate");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");
    let card_path = dir.join("local-peer.card.json");
    let book_path = dir.join("duplicate.contact-book.json");
    let card_path_text = card_path.to_string_lossy().to_string();
    let book_path_text = book_path.to_string_lossy().to_string();

    let card_output = Command::new(minimodel_bin())
        .args([
            "write-peer-contact-card",
            &card_path_text,
            "http://127.0.0.1:8123/peer-pieces",
            "Local CLI peer",
        ])
        .output()
        .expect("run write-peer-contact-card");
    assert!(
        card_output.status.success(),
        "{}",
        output_text(&card_output)
    );

    let output = Command::new(minimodel_bin())
        .args([
            "write-peer-contact-book",
            &book_path_text,
            &card_path_text,
            &card_path_text,
        ])
        .output()
        .expect("run write-peer-contact-book");

    assert!(!output.status.success(), "{}", output_text(&output));
    let text = output_text(&output);
    assert!(text.contains("duplicate peer_base_url"), "{text}");
    assert!(!book_path.exists());
}
