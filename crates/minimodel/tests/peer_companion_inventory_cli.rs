use std::process::Command;

mod common;

use common::{minimodel_bin, output_text, q8_manifest_text, sibling_q8_fixture, unique_dir};

#[test]
fn peer_companion_inventory_cli_round_trips_verified_local_metadata() {
    let Some(artifact_path) = sibling_q8_fixture() else {
        eprintln!("skipping peer companion inventory CLI test; TinyRustLM fixture not present");
        return;
    };
    let dir = unique_dir("peer-companion-inventory-cli");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test directory");

    let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
    let piece_dir = dir.join("tiny-test-model-q8.peer-pieces");
    let state_path = dir.join("tiny-test-model-q8.peer-companion-state.v0.json");
    let receipt_path = dir.join("tiny-test-model-q8.local.minimodel-receipt");
    let ledger_path = dir.join("import-receipts.v0.json");
    let card_path = dir.join("tiny-test-model-q8.peer-contact-card.v0.json");
    let duplicate_card_path = dir.join("tiny-test-model-q8.duplicate-peer-contact-card.v0.json");
    let inventory_path = dir.join("tiny-test-model-q8.peer-companion-inventory.v0.json");
    std::fs::write(&manifest_path, q8_manifest_text()).expect("write q8 manifest");

    let manifest = manifest_path.to_string_lossy().to_string();
    let artifact = artifact_path.to_string_lossy().to_string();
    let piece_dir_text = piece_dir.to_string_lossy().to_string();
    let state = state_path.to_string_lossy().to_string();
    let receipt = receipt_path.to_string_lossy().to_string();
    let ledger = ledger_path.to_string_lossy().to_string();
    let card = card_path.to_string_lossy().to_string();
    let duplicate_card = duplicate_card_path.to_string_lossy().to_string();
    let inventory = inventory_path.to_string_lossy().to_string();

    let export_output = Command::new(minimodel_bin())
        .args(["export-peer-pieces", &manifest, &artifact, &piece_dir_text])
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
            &manifest,
            &piece_dir_text,
            &state,
            "http://127.0.0.1:8123/peer-pieces",
        ])
        .output()
        .expect("run write-peer-companion-state");
    assert!(
        state_output.status.success(),
        "{}",
        output_text(&state_output)
    );

    let receipt_output = Command::new(minimodel_bin())
        .args(["write-import-receipt", &manifest, &artifact, &receipt])
        .output()
        .expect("run write-import-receipt");
    assert!(
        receipt_output.status.success(),
        "{}",
        output_text(&receipt_output)
    );

    let ledger_output = Command::new(minimodel_bin())
        .args([
            "write-import-receipt-ledger",
            &ledger,
            &receipt,
            &manifest,
            &artifact,
        ])
        .output()
        .expect("run write-import-receipt-ledger");
    assert!(
        ledger_output.status.success(),
        "{}",
        output_text(&ledger_output)
    );

    let card_output = Command::new(minimodel_bin())
        .args([
            "write-peer-contact-card",
            &card,
            "http://127.0.0.1:8123/peer-pieces",
            "Local inventory peer",
        ])
        .output()
        .expect("run write-peer-contact-card");
    assert!(
        card_output.status.success(),
        "{}",
        output_text(&card_output)
    );

    let duplicate_card_output = Command::new(minimodel_bin())
        .args([
            "write-peer-contact-card",
            &duplicate_card,
            "http://127.0.0.1:8123/peer-pieces",
            "Duplicate inventory peer",
        ])
        .output()
        .expect("run duplicate write-peer-contact-card");
    assert!(
        duplicate_card_output.status.success(),
        "{}",
        output_text(&duplicate_card_output)
    );

    let inventory_output = Command::new(minimodel_bin())
        .args([
            "write-peer-companion-inventory",
            &inventory,
            &state,
            &manifest,
            &piece_dir_text,
            &ledger,
            "8832",
            &card,
        ])
        .output()
        .expect("run write-peer-companion-inventory");
    assert!(
        inventory_output.status.success(),
        "{}",
        output_text(&inventory_output)
    );
    let inventory_cli_text = output_text(&inventory_output);
    assert!(inventory_cli_text.contains("MiniModel peer companion inventory written"));
    assert!(inventory_cli_text.contains("model.id: tinyrustlm/tiny-test-model-q8"));
    assert!(inventory_cli_text.contains("peer_base_url: http://127.0.0.1:8123/peer-pieces/"));
    assert!(inventory_cli_text.contains("serve_plan.status: ready-after-rust-reverify"));
    assert!(inventory_cli_text.contains("piece_store_byte_count: 8832"));
    assert!(inventory_cli_text.contains("piece_store.sha256: sha256:"));
    assert!(inventory_cli_text.contains("piece_store_quota_bytes: 8832"));
    assert!(inventory_cli_text.contains("receipt_entry_count: 1"));
    assert!(inventory_cli_text.contains("contact_card_count: 1"));

    let inventory_text = std::fs::read_to_string(&inventory_path).expect("read inventory");
    assert!(
        inventory_text.contains("\"inventory_kind\": \"minimodel.peer_companion_inventory.v0\"")
    );
    assert!(inventory_text.contains("\"piece_store_quota_bytes\": 8832"));
    assert!(inventory_text.contains("\"piece_store_sha256\": \"sha256:"));
    assert!(inventory_text.contains("\"quota_status\": \"within-quota\""));
    assert!(inventory_text.contains("\"serve_plan\""));
    assert!(inventory_text.contains("\"status\": \"ready-after-rust-reverify\""));
    assert!(inventory_text.contains("\"peer_status_route\": \"minimodel.peer_status.v0.json\""));
    assert!(inventory_text.contains("\"peer_piece_set_route\": \"minimodel.peer_piece_set.v0\""));
    assert!(inventory_text.contains("\"rust_reverify_required_before_serving\": true"));
    assert!(inventory_text.contains("\"inventory_contains_model_bytes\": false"));
    assert!(inventory_text.contains("\"inventory_contains_peer_piece_bytes\": false"));
    assert!(inventory_text.contains("\"project_server_serves_model_bytes\": false"));
    assert!(!inventory_text.contains("artifact_project_server_url"));

    let inspect_output = Command::new(minimodel_bin())
        .args(["inspect-peer-companion-inventory", &inventory])
        .output()
        .expect("run inspect-peer-companion-inventory");
    assert!(
        inspect_output.status.success(),
        "{}",
        output_text(&inspect_output)
    );
    let inspect_text = output_text(&inspect_output);
    assert!(inspect_text.contains("MiniModel peer companion inventory validation: PASS"));
    assert!(inspect_text.contains("serve_plan.status: ready-after-rust-reverify"));
    assert!(inspect_text.contains("piece_store_byte_count: 8832"));
    assert!(inspect_text.contains("piece_store.sha256: sha256:"));
    assert!(inspect_text.contains("metadata-only peer companion inventory"));

    let duplicate_peer_inventory_path = dir.join("duplicate-peer.inventory.json");
    let duplicate_peer_inventory = duplicate_peer_inventory_path.to_string_lossy().to_string();
    let duplicate_peer_output = Command::new(minimodel_bin())
        .args([
            "write-peer-companion-inventory",
            &duplicate_peer_inventory,
            &state,
            &manifest,
            &piece_dir_text,
            &ledger,
            "8832",
            &card,
            &duplicate_card,
        ])
        .output()
        .expect("run write-peer-companion-inventory with duplicate peer URL cards");
    assert!(
        !duplicate_peer_output.status.success(),
        "{}",
        output_text(&duplicate_peer_output)
    );
    let duplicate_peer_text = output_text(&duplicate_peer_output);
    assert!(
        duplicate_peer_text.contains("duplicate contact card peer_base_url"),
        "{duplicate_peer_text}"
    );
    assert!(!duplicate_peer_inventory_path.exists());

    let manually_duplicated_inventory_path =
        dir.join("manual-duplicate-peer.peer-companion-inventory.v0.json");
    let manually_duplicated_inventory = manually_duplicated_inventory_path
        .to_string_lossy()
        .to_string();
    let card_block_start = inventory_text
        .find("  \"contact_cards\": [\n")
        .expect("inventory has contact cards start");
    let contact_block_header = "  \"contact_cards\": [\n";
    let card_block_end = inventory_text[card_block_start + contact_block_header.len()..]
        .find("\n  ],")
        .map(|offset| card_block_start + contact_block_header.len() + offset)
        .expect("inventory has contact cards end");
    let card_prefix = &inventory_text[..card_block_start];
    let original_card_block =
        &inventory_text[card_block_start + contact_block_header.len()..card_block_end];
    let card_suffix = &inventory_text[card_block_end..];
    let card_json_path = card.replace('\\', "\\\\");
    let duplicate_card_json_path = duplicate_card.replace('\\', "\\\\");
    let duplicate_card_block = original_card_block
        .replace(&card_json_path, &duplicate_card_json_path)
        .replace("Local inventory peer", "Duplicate inventory peer");
    let manually_duplicated_inventory_text = format!(
        "{}{}{},\n{}{}",
        card_prefix.replace("\"contact_card_count\": 1", "\"contact_card_count\": 2"),
        contact_block_header,
        original_card_block,
        duplicate_card_block,
        card_suffix
    );
    std::fs::write(
        &manually_duplicated_inventory_path,
        manually_duplicated_inventory_text,
    )
    .expect("write manually duplicated inventory");
    let manual_duplicate_output = Command::new(minimodel_bin())
        .args([
            "inspect-peer-companion-inventory",
            &manually_duplicated_inventory,
        ])
        .output()
        .expect("run inspect-peer-companion-inventory duplicate peer URL drift");
    assert!(
        !manual_duplicate_output.status.success(),
        "{}",
        output_text(&manual_duplicate_output)
    );
    let manual_duplicate_text = output_text(&manual_duplicate_output);
    assert!(
        manual_duplicate_text.contains("duplicate contact card peer_base_url"),
        "{manual_duplicate_text}"
    );

    let too_small_quota_path = dir.join("too-small-quota.inventory.json");
    let too_small_quota = too_small_quota_path.to_string_lossy().to_string();
    let quota_output = Command::new(minimodel_bin())
        .args([
            "write-peer-companion-inventory",
            &too_small_quota,
            &state,
            &manifest,
            &piece_dir_text,
            &ledger,
            "1",
            &card,
        ])
        .output()
        .expect("run write-peer-companion-inventory with too small quota");
    assert!(
        !quota_output.status.success(),
        "{}",
        output_text(&quota_output)
    );
    let quota_text = output_text(&quota_output);
    assert!(quota_text.contains("exceeds quota"), "{quota_text}");
    assert!(!too_small_quota_path.exists());

    let drifted_inventory_path = dir.join("drifted.peer-companion-inventory.v0.json");
    let drifted_inventory = drifted_inventory_path.to_string_lossy().to_string();
    std::fs::write(
        &drifted_inventory_path,
        inventory_text.replace("\"piece_count\": 1", "\"piece_count\": 2"),
    )
    .expect("write drifted inventory");
    let drift_output = Command::new(minimodel_bin())
        .args(["inspect-peer-companion-inventory", &drifted_inventory])
        .output()
        .expect("run inspect-peer-companion-inventory drift");
    assert!(
        !drift_output.status.success(),
        "{}",
        output_text(&drift_output)
    );
    let drift_text = output_text(&drift_output);
    assert!(drift_text.contains("piece_count mismatch"), "{drift_text}");

    let piece_store_sha_drift_path =
        dir.join("piece-store-sha-drift.peer-companion-inventory.v0.json");
    let piece_store_sha_drift = piece_store_sha_drift_path.to_string_lossy().to_string();
    let piece_store_sha_line = inventory_text
        .lines()
        .find(|line| line.contains("\"piece_store_sha256\""))
        .expect("inventory has piece_store_sha256 line")
        .trim()
        .to_string();
    std::fs::write(
        &piece_store_sha_drift_path,
        inventory_text.replace(
            &piece_store_sha_line,
            "\"piece_store_sha256\": \"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",",
        ),
    )
    .expect("write piece store SHA drifted inventory");
    let piece_store_sha_drift_output = Command::new(minimodel_bin())
        .args(["inspect-peer-companion-inventory", &piece_store_sha_drift])
        .output()
        .expect("run inspect-peer-companion-inventory piece store SHA drift");
    assert!(
        !piece_store_sha_drift_output.status.success(),
        "{}",
        output_text(&piece_store_sha_drift_output)
    );
    let piece_store_sha_drift_text = output_text(&piece_store_sha_drift_output);
    assert!(
        piece_store_sha_drift_text.contains("piece_store_sha256 mismatch"),
        "{piece_store_sha_drift_text}"
    );

    let serve_plan_drift_path = dir.join("serve-plan-drift.peer-companion-inventory.v0.json");
    let serve_plan_drift = serve_plan_drift_path.to_string_lossy().to_string();
    std::fs::write(
        &serve_plan_drift_path,
        inventory_text.replace(
            "\"rust_reverify_required_before_serving\": true",
            "\"rust_reverify_required_before_serving\": false",
        ),
    )
    .expect("write serve plan drifted inventory");
    let serve_plan_drift_output = Command::new(minimodel_bin())
        .args(["inspect-peer-companion-inventory", &serve_plan_drift])
        .output()
        .expect("run inspect-peer-companion-inventory serve plan drift");
    assert!(
        !serve_plan_drift_output.status.success(),
        "{}",
        output_text(&serve_plan_drift_output)
    );
    let serve_plan_drift_text = output_text(&serve_plan_drift_output);
    assert!(
        serve_plan_drift_text.contains("rust_reverify_required_before_serving"),
        "{serve_plan_drift_text}"
    );
    assert!(
        serve_plan_drift_text.contains("must be true"),
        "{serve_plan_drift_text}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
