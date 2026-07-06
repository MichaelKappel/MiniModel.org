use std::env;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Err("missing command".to_string());
    };
    match command.as_str() {
        "verify-manifest" => {
            let manifest = one_arg(&mut args, "manifest")?;
            no_more_args(&mut args)?;
            let parsed = minimodel_core::verify_manifest_path(Path::new(&manifest))?;
            println!("MiniModel manifest validation: PASS");
            println!("manifest: {manifest}");
            println!("model.id: {}", parsed.require("model.id")?);
            println!("artifact.kind: {}", parsed.require("artifact.kind")?);
            println!(
                "artifact.acquisition: {}",
                parsed.require("artifact.acquisition")?
            );
            println!(
                "runtime.compatibility: {}",
                parsed.require("runtime.compatibility")?
            );
            println!(
                "signature.payload_sha256: {}",
                parsed.canonical_signature_payload_sha256()
            );
        }
        "verify-signed-manifest" => {
            let manifest = one_arg(&mut args, "manifest")?;
            let public_key = one_arg(&mut args, "public-key-json")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::verify_signed_manifest_path(
                Path::new(&manifest),
                Path::new(&public_key),
            )?;
            println!("MiniModel signed manifest validation: PASS");
            println!("manifest: {}", report.manifest_path);
            println!("public_key: {}", report.public_key_path);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("signature.algorithm: {}", report.signature_algorithm);
            println!("signature.key_id: {}", report.key_id);
            println!("publisher.id: {}", report.publisher_id);
            println!("signature.public_key_route: {}", report.public_key_route);
            println!("public_key.sha256: {}", report.public_key_sha256);
            println!("signature.payload_sha256: {}", report.payload_sha256);
        }
        "verify-artifact" => {
            let manifest = one_arg(&mut args, "manifest")?;
            let artifact = one_arg(&mut args, "artifact.slm")?;
            no_more_args(&mut args)?;
            let report =
                minimodel_core::verify_artifact_path(Path::new(&manifest), Path::new(&artifact))?;
            println!("MiniModel artifact validation: PASS");
            println!("manifest: {manifest}");
            println!("artifact: {artifact}");
            println!("bytes: {}", report.artifact_byte_count);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("slm.model_shape: {}", report.slm.model_shape);
            println!("slm.quantization: {}", report.slm.quantization);
        }
        "write-import-receipt" => {
            let manifest = one_arg(&mut args, "manifest")?;
            let artifact = one_arg(&mut args, "artifact.slm")?;
            let receipt = one_arg(&mut args, "receipt.minimodel-receipt")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::write_import_receipt_path(
                Path::new(&manifest),
                Path::new(&artifact),
                Path::new(&receipt),
            )?;
            println!("MiniModel import receipt written: {receipt}");
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
        }
        "inspect-import-receipt" => {
            let receipt = one_arg(&mut args, "receipt.minimodel-receipt")?;
            let manifest = one_arg(&mut args, "manifest")?;
            let artifact = one_arg(&mut args, "artifact.slm")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::inspect_import_receipt_path(
                Path::new(&receipt),
                Path::new(&manifest),
                Path::new(&artifact),
            )?;
            println!("MiniModel import receipt validation: PASS");
            println!("receipt: {}", report.receipt_path);
            println!("manifest: {manifest}");
            println!("artifact: {}", report.artifact_path);
            println!("model.id: {}", report.model_id);
            println!("bytes: {}", report.artifact_byte_count);
            println!("receipt.sha256: {}", report.receipt_sha256);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("slm.model_shape: {}", report.slm_model_shape);
            println!("slm.quantization: {}", report.slm_quantization);
            println!("peer_import: {}", report.peer_import);
            if let Some(receiver_validation_contract_status) =
                report.receiver_validation_contract_status
            {
                println!(
                    "receiver_validation_contract.status: {receiver_validation_contract_status}"
                );
                println!(
                    "receiver_validation.check_peer_status_before_piece_fetch: {}",
                    report.check_peer_status_before_piece_fetch
                );
                println!(
                    "receiver_validation.preflight_fetches_piece_set_before_pieces: {}",
                    report.preflight_fetches_piece_set_before_pieces
                );
                println!(
                    "receiver_validation.validate_advertised_companion_state_before_piece_fetch: {}",
                    report.validate_advertised_companion_state_before_piece_fetch
                );
                println!(
                    "receiver_validation.import_receipt_required_before_browser_load: {}",
                    report.import_receipt_required_before_browser_load
                );
            }
            if let Some(peer_base_url) = report.peer_base_url {
                println!("peer_base_url: {peer_base_url}");
            }
            if let Some(companion_state_status) = report.companion_state_status {
                println!("companion_state_status: {companion_state_status}");
            }
            if let Some(piece_count) = report.piece_count {
                println!("piece_count: {piece_count}");
            }
            if let Some(merkle_root_sha256) = report.merkle_root_sha256 {
                println!("chunks.merkle_root_sha256: {merkle_root_sha256}");
            }
            println!(
                "peer_piece_bytes_fetched: {}",
                report.peer_piece_bytes_fetched
            );
            println!(
                "artifact.assembled_from_peer_pieces: {}",
                report.artifact_assembled_from_peer_pieces
            );
        }
        "inspect-peer-import-receipt" => {
            let receipt = one_arg(&mut args, "receipt.minimodel-receipt")?;
            let request_json = one_arg(&mut args, "request-json")?;
            let preflight = one_arg(&mut args, "preflight")?;
            let manifest = one_arg(&mut args, "manifest")?;
            let artifact = one_arg(&mut args, "artifact.slm")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::inspect_peer_import_receipt_path(
                Path::new(&receipt),
                Path::new(&request_json),
                Path::new(&preflight),
                Path::new(&manifest),
                Path::new(&artifact),
            )?;
            println!("MiniModel peer import receipt chain validation: PASS");
            println!("receipt: {}", report.receipt_path);
            println!("request: {}", report.request_path);
            println!("preflight: {}", report.preflight_path);
            println!("manifest: {manifest}");
            println!("artifact: {}", report.artifact_path);
            println!("model.id: {}", report.model_id);
            println!(
                "receiver_validation_contract.status: {}",
                report.receiver_validation_contract_status
            );
            println!(
                "receiver_validation.check_peer_status_before_piece_fetch: {}",
                report.check_peer_status_before_piece_fetch
            );
            println!(
                "receiver_validation.preflight_fetches_piece_set_before_pieces: {}",
                report.preflight_fetches_piece_set_before_pieces
            );
            println!(
                "receiver_validation.validate_advertised_companion_state_before_piece_fetch: {}",
                report.validate_advertised_companion_state_before_piece_fetch
            );
            println!(
                "receiver_validation.import_receipt_required_before_browser_load: {}",
                report.import_receipt_required_before_browser_load
            );
            println!("peer_base_url: {}", report.peer_base_url);
            println!("companion_state_url: {}", report.companion_state_url);
            println!("companion_state_status: {}", report.companion_state_status);
            println!("piece_set.sha256: {}", report.piece_set_sha256);
            println!("piece_count: {}", report.piece_count);
            println!("receipt.sha256: {}", report.receipt_sha256);
            println!("request.sha256: {}", report.request_sha256);
            println!("preflight.sha256: {}", report.preflight_sha256);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
            println!(
                "peer_piece_set_metadata_fetched: {}",
                report.peer_piece_set_metadata_fetched
            );
            println!(
                "peer_companion_state_metadata_fetched: {}",
                report.peer_companion_state_metadata_fetched
            );
            println!(
                "peer_piece_bytes_fetched: {}",
                report.peer_piece_bytes_fetched
            );
            println!(
                "artifact.assembled_from_peer_pieces: {}",
                report.artifact_assembled_from_peer_pieces
            );
            println!(
                "project_server_serves_model_bytes: {}",
                report.project_server_serves_model_bytes
            );
            println!(
                "project_server_runs_model_bytes: {}",
                report.project_server_runs_model_bytes
            );
            println!(
                "project_server_proxies_huggingface_bytes: {}",
                report.project_server_proxies_huggingface_bytes
            );
        }
        "write-import-receipt-ledger" => {
            let output_json = one_arg(&mut args, "output-json")?;
            let entries = receipt_ledger_entries(args.collect())?;
            let report = minimodel_core::write_import_receipt_ledger_path(
                Path::new(&output_json),
                &entries,
            )?;
            println!("MiniModel import receipt ledger written: {output_json}");
            println!("ledger: {}", report.ledger_path);
            println!("entry_count: {}", report.entry_count);
            println!("peer_import_count: {}", report.peer_import_count);
            println!("model_count: {}", report.model_count);
            println!("ledger.sha256: {}", report.ledger_sha256);
        }
        "inspect-import-receipt-ledger" => {
            let ledger_json = one_arg(&mut args, "ledger-json")?;
            no_more_args(&mut args)?;
            let report =
                minimodel_core::inspect_import_receipt_ledger_path(Path::new(&ledger_json))?;
            println!("MiniModel import receipt ledger validation: PASS");
            println!("ledger: {}", report.ledger_path);
            println!("entry_count: {}", report.entry_count);
            println!("peer_import_count: {}", report.peer_import_count);
            println!("model_count: {}", report.model_count);
            println!("ledger.sha256: {}", report.ledger_sha256);
        }
        "export-peer-pieces" => {
            let manifest = one_arg(&mut args, "manifest")?;
            let artifact = one_arg(&mut args, "artifact.slm")?;
            let output_dir = one_arg(&mut args, "output-dir")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::export_peer_pieces_path(
                Path::new(&manifest),
                Path::new(&artifact),
                Path::new(&output_dir),
            )?;
            println!("MiniModel peer pieces exported: {output_dir}");
            println!("piece_set: {}", report.piece_set_path);
            println!("piece_count: {}", report.piece_count);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
        }
        "assemble-peer-pieces" => {
            let manifest = one_arg(&mut args, "manifest")?;
            let piece_dir = one_arg(&mut args, "piece-dir")?;
            let output_artifact = one_arg(&mut args, "output.slm")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::assemble_peer_pieces_path(
                Path::new(&manifest),
                Path::new(&piece_dir),
                Path::new(&output_artifact),
            )?;
            println!("MiniModel peer pieces assembled and verified: {output_artifact}");
            println!("bytes: {}", report.artifact_byte_count);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("slm.model_shape: {}", report.slm.model_shape);
            println!("slm.quantization: {}", report.slm.quantization);
        }
        "fetch-peer-pieces" => {
            let manifest = one_arg(&mut args, "manifest")?;
            let peer_base_url = one_arg(&mut args, "http-peer-base-url")?;
            let output_dir = one_arg(&mut args, "output-dir")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::fetch_peer_pieces_http_path(
                Path::new(&manifest),
                &peer_base_url,
                Path::new(&output_dir),
            )?;
            println!("MiniModel peer pieces fetched and verified: {output_dir}");
            println!("peer_base_url: {}", report.peer_base_url);
            println!("companion_state_url: {}", report.companion_state_url);
            println!("companion_state_status: {}", report.companion_state_status);
            println!("piece_set: {}", report.piece_set_path);
            println!("piece_set.sha256: {}", report.piece_set_sha256);
            println!("piece_count: {}", report.piece_count);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
        }
        "import-peer-request" => {
            let request_json = one_arg(&mut args, "request-json")?;
            let manifest = one_arg(&mut args, "manifest")?;
            let output_dir = one_arg(&mut args, "output-dir")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::execute_peer_import_request_path(
                Path::new(&request_json),
                Path::new(&manifest),
                Path::new(&output_dir),
            )?;
            println!("MiniModel peer import request executed: {request_json}");
            println!("output_dir: {output_dir}");
            println!("peer_base_url: {}", report.peer_base_url);
            println!("companion_state_url: {}", report.companion_state_url);
            println!("companion_state_status: {}", report.companion_state_status);
            println!("piece_dir: {}", report.piece_dir_path);
            println!("piece_set.sha256: {}", report.piece_set_sha256);
            println!("piece_count: {}", report.piece_count);
            println!("artifact: {}", report.output_artifact_path);
            println!("receipt: {}", report.receipt_path);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
        }
        "inspect-peer-import-request" => {
            let request_json = one_arg(&mut args, "request-json")?;
            let manifest = one_arg(&mut args, "manifest")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::inspect_peer_import_request_path(
                Path::new(&request_json),
                Path::new(&manifest),
            )?;
            println!("MiniModel peer import request validation: PASS");
            println!("request: {}", report.request_path);
            println!("manifest: {manifest}");
            println!("model.id: {}", report.model_id);
            println!("artifact.byte_count: {}", report.artifact_byte_count);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("peer_base_url: {}", report.expected_peer_base_url);
            println!("expected_piece_dir: {}", report.expected_piece_dir);
            println!("assembled_output_slm: {}", report.assembled_output_slm);
            println!("metadata_routes.status: {}", report.metadata_routes_status);
            println!("metadata_routes.peer_status: {}", report.peer_status_route);
            println!(
                "metadata_routes.peer_piece_set: {}",
                report.peer_piece_set_route
            );
            println!(
                "metadata_routes.peer_companion_state: {}",
                report.peer_companion_state_route
            );
            println!(
                "receiver_validation_contract.status: {}",
                report.receiver_validation_contract_status
            );
            println!(
                "receiver_validation.check_peer_status_before_piece_fetch: {}",
                report.check_peer_status_before_piece_fetch
            );
            println!(
                "receiver_validation.preflight_fetches_piece_set_before_pieces: {}",
                report.preflight_fetches_piece_set_before_pieces
            );
            println!(
                "receiver_validation.validate_advertised_companion_state_before_piece_fetch: {}",
                report.validate_advertised_companion_state_before_piece_fetch
            );
            println!(
                "receiver_validation.import_receipt_required_before_browser_load: {}",
                report.import_receipt_required_before_browser_load
            );
            println!("request.sha256: {}", report.request_sha256);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("peer_piece_bytes_fetched: false");
            println!("artifact_bytes_fetched: false");
        }
        "preflight-peer-import-request" => {
            let request_json = one_arg(&mut args, "request-json")?;
            let manifest = one_arg(&mut args, "manifest")?;
            let output_preflight = one_arg(&mut args, "output.preflight")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::preflight_peer_import_request_path(
                Path::new(&request_json),
                Path::new(&manifest),
                Path::new(&output_preflight),
            )?;
            println!("MiniModel peer import preflight: PASS");
            println!("request: {request_json}");
            println!("manifest: {manifest}");
            println!("preflight: {output_preflight}");
            println!("peer_base_url: {}", report.peer_base_url);
            println!("metadata_routes.status: {}", report.metadata_routes_status);
            println!("metadata_routes.peer_status: {}", report.peer_status_route);
            println!(
                "metadata_routes.peer_piece_set: {}",
                report.peer_piece_set_route
            );
            println!(
                "metadata_routes.peer_companion_state: {}",
                report.peer_companion_state_route
            );
            println!(
                "receiver_validation_contract.status: {}",
                report.receiver_validation_contract_status
            );
            println!(
                "receiver_validation.check_peer_status_before_piece_fetch: {}",
                report.check_peer_status_before_piece_fetch
            );
            println!(
                "receiver_validation.preflight_fetches_piece_set_before_pieces: {}",
                report.preflight_fetches_piece_set_before_pieces
            );
            println!(
                "receiver_validation.validate_advertised_companion_state_before_piece_fetch: {}",
                report.validate_advertised_companion_state_before_piece_fetch
            );
            println!(
                "receiver_validation.import_receipt_required_before_browser_load: {}",
                report.import_receipt_required_before_browser_load
            );
            println!("piece_set_url: {}", report.piece_set_url);
            println!("piece_set.sha256: {}", report.piece_set_sha256);
            println!("companion_state_url: {}", report.companion_state_url);
            println!("companion_state_status: {}", report.companion_state_status);
            println!("piece_count: {}", report.piece_count);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
            println!("peer_piece_bytes_fetched: false");
        }
        "inspect-peer-import-preflight" => {
            let preflight = one_arg(&mut args, "preflight")?;
            let request_json = one_arg(&mut args, "request-json")?;
            let manifest = one_arg(&mut args, "manifest")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::inspect_peer_import_preflight_path(
                Path::new(&preflight),
                Path::new(&request_json),
                Path::new(&manifest),
            )?;
            println!("MiniModel peer import preflight validation: PASS");
            println!("preflight: {}", report.preflight_path);
            println!("request: {}", report.request_path);
            println!("manifest: {manifest}");
            println!("model.id: {}", report.model_id);
            println!("peer_base_url: {}", report.peer_base_url);
            println!("metadata_routes.status: {}", report.metadata_routes_status);
            println!("metadata_routes.peer_status: {}", report.peer_status_route);
            println!(
                "metadata_routes.peer_piece_set: {}",
                report.peer_piece_set_route
            );
            println!(
                "metadata_routes.peer_companion_state: {}",
                report.peer_companion_state_route
            );
            println!(
                "receiver_validation_contract.status: {}",
                report.receiver_validation_contract_status
            );
            println!(
                "receiver_validation.check_peer_status_before_piece_fetch: {}",
                report.check_peer_status_before_piece_fetch
            );
            println!(
                "receiver_validation.preflight_fetches_piece_set_before_pieces: {}",
                report.preflight_fetches_piece_set_before_pieces
            );
            println!(
                "receiver_validation.validate_advertised_companion_state_before_piece_fetch: {}",
                report.validate_advertised_companion_state_before_piece_fetch
            );
            println!(
                "receiver_validation.import_receipt_required_before_browser_load: {}",
                report.import_receipt_required_before_browser_load
            );
            println!("piece_set_url: {}", report.piece_set_url);
            println!("piece_set.sha256: {}", report.piece_set_sha256);
            println!("companion_state_url: {}", report.companion_state_url);
            println!("companion_state_status: {}", report.companion_state_status);
            println!("piece_count: {}", report.piece_count);
            println!("preflight.sha256: {}", report.preflight_sha256);
            println!("request.sha256: {}", report.request_sha256);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
            println!(
                "peer_piece_set_metadata_fetched: {}",
                report.peer_piece_set_metadata_fetched
            );
            println!(
                "peer_companion_state_metadata_fetched: {}",
                report.peer_companion_state_metadata_fetched
            );
            println!(
                "peer_piece_bytes_fetched: {}",
                report.peer_piece_bytes_fetched
            );
            println!("artifact_bytes_fetched: {}", report.artifact_bytes_fetched);
            println!(
                "project_server_serves_model_bytes: {}",
                report.project_server_serves_model_bytes
            );
            println!(
                "project_server_runs_model_bytes: {}",
                report.project_server_runs_model_bytes
            );
            println!(
                "project_server_proxies_huggingface_bytes: {}",
                report.project_server_proxies_huggingface_bytes
            );
        }
        "check-peer-status" => {
            let manifest = one_arg(&mut args, "manifest")?;
            let peer_base_url = one_arg(&mut args, "known-peer-base-url")?;
            let output_json = one_arg(&mut args, "output-json")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::check_peer_status_path(
                Path::new(&manifest),
                &peer_base_url,
                Path::new(&output_json),
            )?;
            println!("MiniModel peer status check: PASS");
            println!("manifest: {manifest}");
            println!("peer_base_url: {}", report.peer_base_url);
            println!("status_url: {}", report.status_url);
            println!("status_report: {}", report.report_path);
            println!("model.id: {}", report.model_id);
            println!("artifact.byte_count: {}", report.artifact_byte_count);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("piece_set.sha256: {}", report.piece_set_sha256);
            println!("piece_count: {}", report.piece_count);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
            println!("companion_state_route: {}", report.companion_state_route);
            println!("peer_piece_bytes_fetched: false");
            println!("artifact_bytes_fetched: false");
        }
        "inspect-peer-status-check" => {
            let status_check_json = one_arg(&mut args, "status-check-json")?;
            let manifest = one_arg(&mut args, "manifest")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::inspect_peer_status_check_path(
                Path::new(&status_check_json),
                Path::new(&manifest),
            )?;
            println!("MiniModel peer status check validation: PASS");
            println!("status_check: {}", report.check_path);
            println!("manifest: {manifest}");
            println!("peer_base_url: {}", report.peer_base_url);
            println!("status_url: {}", report.status_url);
            println!("model.id: {}", report.model_id);
            println!("checked_utc: {}", report.checked_utc);
            println!("artifact.byte_count: {}", report.artifact_byte_count);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("piece_set.sha256: {}", report.piece_set_sha256);
            println!("piece_count: {}", report.piece_count);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
            println!("companion_state_route: {}", report.companion_state_route);
            println!("status_check.sha256: {}", report.check_sha256);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("peer_piece_bytes_fetched: false");
            println!("artifact_bytes_fetched: false");
            println!("boundary: saved peer-status metadata only; Rust reverify still required before browser import");
        }
        "prepare-peer-share" => {
            let artifact = one_arg(&mut args, "artifact.slm")?;
            let output_dir = one_arg(&mut args, "output-dir")?;
            let model_id = one_arg(&mut args, "model-id")?;
            let optional_first = args.next();
            let optional_second = args.next();
            no_more_args(&mut args)?;
            let (source_huggingface_model_id, known_peer_base_url) =
                split_catalog_entry_optional_args(optional_first, optional_second);
            let report = minimodel_core::prepare_peer_share_path(
                Path::new(&artifact),
                Path::new(&output_dir),
                &model_id,
                source_huggingface_model_id.as_deref(),
                known_peer_base_url.as_deref(),
            )?;
            println!("MiniModel peer share prepared: {output_dir}");
            println!("model.id: {}", report.model_id);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("manifest: {}", report.manifest_path);
            println!("peer_offer: {}", report.peer_offer_path);
            println!("peer_pieces: {}", report.piece_dir_path);
            println!("piece_count: {}", report.piece_count);
            println!("catalog_entry: {}", report.catalog_entry_path);
            println!("peer_source_count: {}", report.peer_source_count);
            println!(
                "catalog_snapshot_hint: minimodel write-catalog-snapshot models.v0.json \"{}\"",
                report.catalog_entry_path
            );
            println!(
                "serve_hint: minimodel serve-peer-pieces \"{}\" \"{}\" 127.0.0.1:8123",
                report.manifest_path, report.piece_dir_path
            );
        }
        "serve-peer-pieces" => {
            let manifest = one_arg(&mut args, "manifest")?;
            let piece_dir = one_arg(&mut args, "piece-dir")?;
            let bind_addr = one_arg(&mut args, "bind-addr")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::inspect_peer_piece_dir_path(
                Path::new(&manifest),
                Path::new(&piece_dir),
            )?;
            println!("MiniModel peer pieces ready to serve: {piece_dir}");
            println!("bind_addr: {bind_addr}");
            println!("peer_base_url: http://{bind_addr}/");
            println!("piece_set: {}", report.piece_set_path);
            println!("piece_count: {}", report.piece_count);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
            println!("Press Ctrl+C to stop serving peer pieces.");
            minimodel_core::serve_peer_pieces_http_path(
                Path::new(&manifest),
                Path::new(&piece_dir),
                &bind_addr,
            )?;
        }
        "serve-peer-companion" => {
            let state_json = one_arg(&mut args, "state-json")?;
            let manifest = one_arg(&mut args, "manifest")?;
            let piece_dir = one_arg(&mut args, "piece-dir")?;
            let bind_addr = one_arg(&mut args, "bind-addr")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::inspect_peer_companion_state_path(
                Path::new(&state_json),
                Path::new(&manifest),
                Path::new(&piece_dir),
            )?;
            println!("MiniModel peer companion ready to serve: {state_json}");
            println!("bind_addr: {bind_addr}");
            println!("peer_base_url: {}", report.peer_base_url);
            println!("state_route: {}", report.state_route);
            println!("piece_set: {}", report.piece_set_path);
            println!("piece_count: {}", report.piece_count);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
            println!("Press Ctrl+C to stop serving the peer companion endpoint.");
            minimodel_core::serve_peer_companion_http_path(
                Path::new(&state_json),
                Path::new(&manifest),
                Path::new(&piece_dir),
                &bind_addr,
            )?;
        }
        "inspect-peer-companion-state" => {
            let state_json = one_arg(&mut args, "state-json")?;
            let manifest = one_arg(&mut args, "manifest")?;
            let piece_dir = one_arg(&mut args, "piece-dir")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::inspect_peer_companion_state_path(
                Path::new(&state_json),
                Path::new(&manifest),
                Path::new(&piece_dir),
            )?;
            println!("MiniModel peer companion state inspection: PASS");
            println!("state_json: {state_json}");
            println!("model.id: {}", report.model_id);
            println!("peer_base_url: {}", report.peer_base_url);
            println!("state_route: {}", report.state_route);
            println!("piece_set: {}", report.piece_set_path);
            println!("piece_set.sha256: {}", report.piece_set_sha256);
            println!("piece_count: {}", report.piece_count);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("artifact.byte_count: {}", report.artifact_byte_count);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
            println!("boundary: metadata-only peer companion state; no model bytes embedded");
        }
        "write-peer-companion-state" => {
            let manifest = one_arg(&mut args, "manifest")?;
            let piece_dir = one_arg(&mut args, "piece-dir")?;
            let output_json = one_arg(&mut args, "output-json")?;
            let known_peer_base_url = one_arg(&mut args, "known-peer-base-url")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::write_peer_companion_state_path(
                Path::new(&manifest),
                Path::new(&piece_dir),
                Path::new(&output_json),
                &known_peer_base_url,
            )?;
            println!("MiniModel peer companion state written: {output_json}");
            println!("model.id: {}", report.model_id);
            println!("peer_base_url: {}", report.peer_base_url);
            println!("piece_count: {}", report.piece_count);
            println!("manifest.sha256: {}", report.manifest_sha256);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
        }
        "write-peer-contact-card" => {
            let output_json = one_arg(&mut args, "output-json")?;
            let known_peer_base_url = one_arg(&mut args, "known-peer-base-url")?;
            let peer_label = args.next();
            no_more_args(&mut args)?;
            let report = minimodel_core::write_peer_contact_card_path(
                Path::new(&output_json),
                &known_peer_base_url,
                peer_label.as_deref(),
            )?;
            println!("MiniModel peer contact card written: {output_json}");
            println!("card: {}", report.card_path);
            println!("peer_label: {}", report.peer_label);
            println!("peer_base_url: {}", report.peer_base_url);
        }
        "inspect-peer-contact-card" => {
            let card_json = one_arg(&mut args, "card-json")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::inspect_peer_contact_card_path(Path::new(&card_json))?;
            println!("MiniModel peer contact card validation: PASS");
            println!("card: {}", report.card_path);
            println!("peer_label: {}", report.peer_label);
            println!("peer_base_url: {}", report.peer_base_url);
        }
        "write-peer-contact-book" => {
            let output_json = one_arg(&mut args, "output-json")?;
            let contact_cards: Vec<PathBuf> = args.map(PathBuf::from).collect();
            let report = minimodel_core::write_peer_contact_book_path(
                Path::new(&output_json),
                &contact_cards,
            )?;
            println!("MiniModel peer contact book written: {output_json}");
            println!("book: {}", report.book_path);
            println!("contact_card_count: {}", report.contact_card_count);
            println!("first_peer_label: {}", report.first_peer_label);
            println!("first_peer_base_url: {}", report.first_peer_base_url);
            println!("book.sha256: {}", report.book_sha256);
        }
        "inspect-peer-contact-book" => {
            let book_json = one_arg(&mut args, "book-json")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::inspect_peer_contact_book_path(Path::new(&book_json))?;
            println!("MiniModel peer contact book validation: PASS");
            println!("book: {}", report.book_path);
            println!("contact_card_count: {}", report.contact_card_count);
            println!("first_peer_label: {}", report.first_peer_label);
            println!("first_peer_base_url: {}", report.first_peer_base_url);
            println!("book.sha256: {}", report.book_sha256);
            println!(
                "boundary: metadata-only peer contact book; no model or peer-piece bytes embedded"
            );
        }
        "write-peer-companion-inventory" => {
            let output_json = one_arg(&mut args, "output-json")?;
            let state_json = one_arg(&mut args, "state-json")?;
            let manifest = one_arg(&mut args, "manifest")?;
            let piece_dir = one_arg(&mut args, "piece-dir")?;
            let receipt_ledger_json = one_arg(&mut args, "receipt-ledger-json")?;
            let piece_store_quota_bytes = parse_u64_arg(
                &one_arg(&mut args, "piece-store-quota-bytes")?,
                "piece-store-quota-bytes",
            )?;
            let contact_cards: Vec<PathBuf> = args.map(PathBuf::from).collect();
            let report = minimodel_core::write_peer_companion_inventory_path(
                Path::new(&output_json),
                Path::new(&state_json),
                Path::new(&manifest),
                Path::new(&piece_dir),
                Path::new(&receipt_ledger_json),
                piece_store_quota_bytes,
                &contact_cards,
            )?;
            println!("MiniModel peer companion inventory written: {output_json}");
            println!("inventory: {}", report.inventory_path);
            println!("model.id: {}", report.model_id);
            println!("peer_base_url: {}", report.peer_base_url);
            println!("serve_plan.status: {}", report.serve_plan_status);
            println!("piece_count: {}", report.piece_count);
            println!("piece_store_byte_count: {}", report.piece_store_byte_count);
            println!("piece_store.sha256: {}", report.piece_store_sha256);
            println!(
                "piece_store_quota_bytes: {}",
                report.piece_store_quota_bytes
            );
            println!("receipt_entry_count: {}", report.receipt_entry_count);
            println!("contact_card_count: {}", report.contact_card_count);
            println!("inventory.sha256: {}", report.inventory_sha256);
        }
        "inspect-peer-companion-inventory" => {
            let inventory_json = one_arg(&mut args, "inventory-json")?;
            no_more_args(&mut args)?;
            let report =
                minimodel_core::inspect_peer_companion_inventory_path(Path::new(&inventory_json))?;
            println!("MiniModel peer companion inventory validation: PASS");
            println!("inventory: {}", report.inventory_path);
            println!("model.id: {}", report.model_id);
            println!("peer_base_url: {}", report.peer_base_url);
            println!("serve_plan.status: {}", report.serve_plan_status);
            println!("piece_count: {}", report.piece_count);
            println!("piece_store_byte_count: {}", report.piece_store_byte_count);
            println!("piece_store.sha256: {}", report.piece_store_sha256);
            println!(
                "piece_store_quota_bytes: {}",
                report.piece_store_quota_bytes
            );
            println!("receipt_entry_count: {}", report.receipt_entry_count);
            println!("contact_card_count: {}", report.contact_card_count);
            println!("inventory.sha256: {}", report.inventory_sha256);
            println!("boundary: metadata-only peer companion inventory; no model or peer-piece bytes embedded");
        }
        "inspect-share-metadata" => {
            let bundle_json = one_arg(&mut args, "bundle-json")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::inspect_share_metadata_path(Path::new(&bundle_json))?;
            println!("MiniModel share metadata validation: PASS");
            println!("bundle: {}", report.bundle_path);
            println!("model.id: {}", report.model_id);
            println!("artifact_file_name: {}", report.artifact_file_name);
            println!("artifact.byte_count: {}", report.artifact_byte_count);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("chunk_count: {}", report.chunk_count);
            println!("chunks.merkle_root_sha256: {}", report.merkle_root_sha256);
            println!("peer_transfer_status: {}", report.peer_transfer_status);
            println!(
                "peer_transfer_implementation: {}",
                report.peer_transfer_implementation
            );
            println!("share_kit_available: {}", report.share_kit_available);
            println!(
                "network_transfer_available: {}",
                report.network_transfer_available
            );
        }
        "write-catalog-entry" => {
            let manifest = one_arg(&mut args, "manifest")?;
            let peer_offer = one_arg(&mut args, "peer-offer")?;
            let output_json = one_arg(&mut args, "output-json")?;
            let optional_first = args.next();
            let optional_second = args.next();
            no_more_args(&mut args)?;
            let (source_huggingface_model_id, known_peer_base_url) =
                split_catalog_entry_optional_args(optional_first, optional_second);
            let report = minimodel_core::write_catalog_entry_path(
                Path::new(&manifest),
                Path::new(&peer_offer),
                Path::new(&output_json),
                source_huggingface_model_id.as_deref(),
                known_peer_base_url.as_deref(),
            )?;
            println!("MiniModel catalog entry written: {output_json}");
            println!("model.id: {}", report.model_id);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("peer_offer.sha256: {}", report.peer_offer_sha256);
            println!("catalog_entry: {}", report.catalog_entry_path);
        }
        "add-peer-source" => {
            let catalog_entry_json = one_arg(&mut args, "catalog-entry-json")?;
            let output_json = one_arg(&mut args, "output-json")?;
            let known_peer_base_url = one_arg(&mut args, "known-peer-base-url")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::add_peer_source_to_catalog_entry_path(
                Path::new(&catalog_entry_json),
                Path::new(&output_json),
                &known_peer_base_url,
            )?;
            println!("MiniModel peer source attached: {output_json}");
            println!("source_catalog_entry: {}", report.catalog_entry_path);
            println!("catalog_entry: {}", report.output_path);
            println!("model.id: {}", report.model_id);
            println!("peer_base_url: {}", report.peer_base_url);
            println!("peer_source_count: {}", report.peer_source_count);
            println!(
                "catalog_snapshot_hint: minimodel write-catalog-snapshot models.v0.json \"{}\"",
                report.output_path
            );
        }
        "write-peer-source-announcement" => {
            let catalog_entry_json = one_arg(&mut args, "catalog-entry-json")?;
            let output_json = one_arg(&mut args, "output-json")?;
            let known_peer_base_url = one_arg(&mut args, "known-peer-base-url")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::write_peer_source_announcement_path(
                Path::new(&catalog_entry_json),
                Path::new(&output_json),
                &known_peer_base_url,
            )?;
            println!("MiniModel peer source announcement written: {output_json}");
            println!("source_catalog_entry: {}", report.catalog_entry_path);
            println!("announcement: {}", report.announcement_path);
            println!("model.id: {}", report.model_id);
            println!("artifact.sha256: {}", report.artifact_sha256);
            println!("peer_offer.sha256: {}", report.peer_offer_sha256);
            println!("peer_base_url: {}", report.peer_base_url);
        }
        "apply-peer-source-announcements" => {
            let catalog_entry_json = one_arg(&mut args, "catalog-entry-json")?;
            let output_json = one_arg(&mut args, "output-json")?;
            let announcement_paths: Vec<PathBuf> = args.map(PathBuf::from).collect();
            let report = minimodel_core::apply_peer_source_announcements_path(
                Path::new(&catalog_entry_json),
                Path::new(&output_json),
                &announcement_paths,
            )?;
            println!("MiniModel peer source announcements applied: {output_json}");
            println!("source_catalog_entry: {}", report.catalog_entry_path);
            println!("catalog_entry: {}", report.output_path);
            println!("model.id: {}", report.model_id);
            println!(
                "applied_announcement_count: {}",
                report.applied_announcement_count
            );
            println!("peer_source_count: {}", report.peer_source_count);
            println!(
                "catalog_snapshot_hint: minimodel write-catalog-snapshot models.v0.json \"{}\"",
                report.output_path
            );
        }
        "write-catalog-snapshot" => {
            let output_json = one_arg(&mut args, "output-json")?;
            let entry_paths: Vec<PathBuf> = args.map(PathBuf::from).collect();
            let report =
                minimodel_core::write_catalog_snapshot_path(Path::new(&output_json), &entry_paths)?;
            println!("MiniModel catalog snapshot written: {output_json}");
            println!("catalog: {}", report.catalog_path);
            println!("entry_count: {}", report.entry_count);
            println!("p2p_ready_count: {}", report.p2p_ready_count);
            println!("peer_source_count: {}", report.peer_source_count);
        }
        "inspect-catalog-snapshot" => {
            let catalog_json = one_arg(&mut args, "catalog-json")?;
            no_more_args(&mut args)?;
            let report = minimodel_core::inspect_catalog_snapshot_path(Path::new(&catalog_json))?;
            println!("MiniModel catalog snapshot validation: PASS");
            println!("catalog: {}", report.catalog_path);
            println!("entry_count: {}", report.entry_count);
            println!("p2p_ready_count: {}", report.p2p_ready_count);
            println!("peer_source_count: {}", report.peer_source_count);
            println!(
                "announcement_route_count: {}",
                report.announcement_route_count
            );
            println!("catalog.bytes: {}", report.catalog_byte_count);
            println!("catalog.sha256: {}", report.catalog_sha256);
        }
        "write-catalog-snapshot-with-announcements" => {
            let output_json = one_arg(&mut args, "output-json")?;
            let rest: Vec<String> = args.collect();
            let (entry_paths, announcement_paths) = split_catalog_snapshot_announcement_args(rest)?;
            let report = minimodel_core::write_catalog_snapshot_with_announcements_path(
                Path::new(&output_json),
                &entry_paths,
                &announcement_paths,
            )?;
            println!("MiniModel catalog snapshot written with announcements: {output_json}");
            println!("catalog: {}", report.catalog_path);
            println!("entry_count: {}", report.entry_count);
            println!("announcement_count: {}", report.announcement_count);
            println!(
                "applied_announcement_count: {}",
                report.applied_announcement_count
            );
            println!("p2p_ready_count: {}", report.p2p_ready_count);
            println!("peer_source_count: {}", report.peer_source_count);
        }
        "write-peer-source-announcement-feed" => {
            let output_json = one_arg(&mut args, "output-json")?;
            let announcement_paths: Vec<PathBuf> = args.map(PathBuf::from).collect();
            let report = minimodel_core::write_peer_source_announcement_feed_path(
                Path::new(&output_json),
                &announcement_paths,
            )?;
            println!("MiniModel peer source announcement feed written: {output_json}");
            println!("feed: {}", report.feed_path);
            println!("announcement_count: {}", report.announcement_count);
            println!("model_count: {}", report.model_count);
            println!("feed.bytes: {}", report.feed_byte_count);
            println!("feed.sha256: {}", report.feed_sha256);
            println!(
                "catalog_route_hint: minimodel write-catalog-snapshot-with-announcement-feed models.v0.json peer-source-announcements.v0.json \"{}\" <catalog-entry-json>...",
                report.feed_path
            );
        }
        "inspect-peer-source-announcement-feed" => {
            let feed_json = one_arg(&mut args, "feed-json")?;
            no_more_args(&mut args)?;
            let report =
                minimodel_core::inspect_peer_source_announcement_feed_path(Path::new(&feed_json))?;
            println!("MiniModel peer source announcement feed validation: PASS");
            println!("feed: {}", report.feed_path);
            println!("announcement_count: {}", report.announcement_count);
            println!("model_count: {}", report.model_count);
            println!("feed.bytes: {}", report.feed_byte_count);
            println!("feed.sha256: {}", report.feed_sha256);
        }
        "write-catalog-snapshot-with-announcement-feed" => {
            let output_json = one_arg(&mut args, "output-json")?;
            let feed_route = one_arg(&mut args, "feed-route")?;
            let feed_json = one_arg(&mut args, "announcement-feed-json")?;
            let entry_paths: Vec<PathBuf> = args.map(PathBuf::from).collect();
            let report = minimodel_core::write_catalog_snapshot_with_announcement_feed_path(
                Path::new(&output_json),
                &feed_route,
                Path::new(&feed_json),
                &entry_paths,
            )?;
            println!("MiniModel catalog snapshot written with announcement feed: {output_json}");
            println!("catalog: {}", report.catalog_path);
            println!("entry_count: {}", report.entry_count);
            println!("announcement_count: {}", report.announcement_count);
            println!(
                "announcement_model_count: {}",
                report.announcement_model_count
            );
            println!(
                "announcement_feed_route: {}",
                report.announcement_feed_route
            );
            println!(
                "announcement_feed_sha256: {}",
                report.announcement_feed_sha256
            );
            println!("p2p_ready_count: {}", report.p2p_ready_count);
            println!("peer_source_count: {}", report.peer_source_count);
        }
        _ => {
            print_usage();
            return Err(format!("unsupported command {command}"));
        }
    }
    Ok(())
}

fn one_arg(args: &mut impl Iterator<Item = String>, label: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing {label} argument"))
}

fn no_more_args(args: &mut impl Iterator<Item = String>) -> Result<(), String> {
    if let Some(extra) = args.next() {
        return Err(format!("unexpected extra argument {extra}"));
    }
    Ok(())
}

fn parse_u64_arg(value: &str, label: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("{label} must be a non-negative decimal integer"))
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  minimodel verify-manifest <manifest>");
    eprintln!("  minimodel verify-signed-manifest <manifest> <public-key-json>");
    eprintln!("  minimodel verify-artifact <manifest> <artifact.slm>");
    eprintln!(
        "  minimodel write-import-receipt <manifest> <artifact.slm> <receipt.minimodel-receipt>"
    );
    eprintln!(
        "  minimodel inspect-import-receipt <receipt.minimodel-receipt> <manifest> <artifact.slm>"
    );
    eprintln!(
        "  minimodel inspect-peer-import-receipt <receipt.minimodel-receipt> <request-json> <preflight> <manifest> <artifact.slm>"
    );
    eprintln!(
        "  minimodel write-import-receipt-ledger <output-json> <receipt> <manifest> <artifact.slm> [<receipt> <manifest> <artifact.slm>...]"
    );
    eprintln!("  minimodel inspect-import-receipt-ledger <ledger-json>");
    eprintln!("  minimodel export-peer-pieces <manifest> <artifact.slm> <output-dir>");
    eprintln!("  minimodel assemble-peer-pieces <manifest> <piece-dir> <output.slm>");
    eprintln!("  minimodel fetch-peer-pieces <manifest> <http-peer-base-url> <output-dir>");
    eprintln!("  minimodel import-peer-request <request-json> <manifest> <output-dir>");
    eprintln!("  minimodel inspect-peer-import-request <request-json> <manifest>");
    eprintln!(
        "  minimodel preflight-peer-import-request <request-json> <manifest> <output.preflight>"
    );
    eprintln!("  minimodel inspect-peer-import-preflight <preflight> <request-json> <manifest>");
    eprintln!("  minimodel check-peer-status <manifest> <known-peer-base-url> <output-json>");
    eprintln!("  minimodel inspect-peer-status-check <status-check-json> <manifest>");
    eprintln!(
        "  minimodel prepare-peer-share <artifact.slm> <output-dir> <model-id> [source-huggingface-model-id] [known-peer-base-url]"
    );
    eprintln!("  minimodel serve-peer-pieces <manifest> <piece-dir> <bind-addr>");
    eprintln!("  minimodel serve-peer-companion <state-json> <manifest> <piece-dir> <bind-addr>");
    eprintln!("  minimodel inspect-peer-companion-state <state-json> <manifest> <piece-dir>");
    eprintln!(
        "  minimodel write-peer-companion-state <manifest> <piece-dir> <output-json> <known-peer-base-url>"
    );
    eprintln!(
        "  minimodel write-peer-contact-card <output-json> <known-peer-base-url> [peer-label]"
    );
    eprintln!("  minimodel inspect-peer-contact-card <card-json>");
    eprintln!("  minimodel write-peer-contact-book <output-json> <contact-card-json>...");
    eprintln!("  minimodel inspect-peer-contact-book <book-json>");
    eprintln!(
        "  minimodel write-peer-companion-inventory <output-json> <state-json> <manifest> <piece-dir> <receipt-ledger-json> <piece-store-quota-bytes> [contact-card-json...]"
    );
    eprintln!("  minimodel inspect-peer-companion-inventory <inventory-json>");
    eprintln!("  minimodel inspect-share-metadata <bundle-json>");
    eprintln!(
        "  minimodel write-catalog-entry <manifest> <peer-offer> <output-json> [source-huggingface-model-id] [known-peer-base-url]"
    );
    eprintln!(
        "  minimodel add-peer-source <catalog-entry-json> <output-json> <known-peer-base-url>"
    );
    eprintln!(
        "  minimodel write-peer-source-announcement <catalog-entry-json> <output-json> <known-peer-base-url>"
    );
    eprintln!(
        "  minimodel apply-peer-source-announcements <catalog-entry-json> <output-json> <announcement-json>..."
    );
    eprintln!("  minimodel write-catalog-snapshot <output-json> <catalog-entry-json>...");
    eprintln!(
        "  minimodel write-catalog-snapshot-with-announcements <output-json> <catalog-entry-json>... --announcements <announcement-json>..."
    );
    eprintln!(
        "  minimodel write-peer-source-announcement-feed <output-json> [announcement-json...]"
    );
    eprintln!("  minimodel inspect-peer-source-announcement-feed <feed-json>");
    eprintln!("  minimodel inspect-catalog-snapshot <catalog-json>");
    eprintln!(
        "  minimodel write-catalog-snapshot-with-announcement-feed <output-json> <feed-route> <announcement-feed-json> <catalog-entry-json>..."
    );
}

fn split_catalog_entry_optional_args(
    first: Option<String>,
    second: Option<String>,
) -> (Option<String>, Option<String>) {
    match (first, second) {
        (Some(value), None) if is_peer_url_arg(&value) => (None, Some(value)),
        (first, second) => (first, second),
    }
}

fn is_peer_url_arg(value: &str) -> bool {
    value.trim_start().starts_with("http://")
}

fn receipt_ledger_entries(args: Vec<String>) -> Result<Vec<(PathBuf, PathBuf, PathBuf)>, String> {
    if args.is_empty() {
        return Err(
            "write-import-receipt-ledger requires at least one <receipt> <manifest> <artifact.slm> triple"
                .to_string(),
        );
    }
    if !args.len().is_multiple_of(3) {
        return Err(
            "write-import-receipt-ledger entries must be <receipt> <manifest> <artifact.slm> triples"
                .to_string(),
        );
    }
    Ok(args
        .chunks_exact(3)
        .map(|chunk| {
            (
                PathBuf::from(&chunk[0]),
                PathBuf::from(&chunk[1]),
                PathBuf::from(&chunk[2]),
            )
        })
        .collect())
}

fn split_catalog_snapshot_announcement_args(
    args: Vec<String>,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let Some(marker_index) = args.iter().position(|arg| arg == "--announcements") else {
        return Err(
            "write-catalog-snapshot-with-announcements requires --announcements before announcement JSON paths"
                .to_string(),
        );
    };
    let entry_paths: Vec<PathBuf> = args[..marker_index].iter().map(PathBuf::from).collect();
    let announcement_paths: Vec<PathBuf> =
        args[marker_index + 1..].iter().map(PathBuf::from).collect();
    if entry_paths.is_empty() {
        return Err("missing catalog-entry-json before --announcements".to_string());
    }
    if announcement_paths.is_empty() {
        return Err("missing announcement-json after --announcements".to_string());
    }
    Ok((entry_paths, announcement_paths))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_entry_optional_args_accept_source_only() {
        let (source, peer) =
            split_catalog_entry_optional_args(Some("example-org/source-model".to_string()), None);

        assert_eq!(source.as_deref(), Some("example-org/source-model"));
        assert_eq!(peer, None);
    }

    #[test]
    fn catalog_entry_optional_args_accept_peer_only() {
        let (source, peer) =
            split_catalog_entry_optional_args(Some("http://127.0.0.1:8123/".to_string()), None);

        assert_eq!(source, None);
        assert_eq!(peer.as_deref(), Some("http://127.0.0.1:8123/"));
    }

    #[test]
    fn catalog_entry_optional_args_accept_source_and_peer() {
        let (source, peer) = split_catalog_entry_optional_args(
            Some("example-org/source-model".to_string()),
            Some("http://127.0.0.1:8123/".to_string()),
        );

        assert_eq!(source.as_deref(), Some("example-org/source-model"));
        assert_eq!(peer.as_deref(), Some("http://127.0.0.1:8123/"));
    }

    #[test]
    fn catalog_snapshot_announcement_args_split_entries_and_announcements() {
        let (entries, announcements) = split_catalog_snapshot_announcement_args(vec![
            "entry-a.json".to_string(),
            "entry-b.json".to_string(),
            "--announcements".to_string(),
            "announcement-a.json".to_string(),
            "announcement-b.json".to_string(),
        ])
        .unwrap();

        assert_eq!(
            entries,
            vec![PathBuf::from("entry-a.json"), PathBuf::from("entry-b.json")]
        );
        assert_eq!(
            announcements,
            vec![
                PathBuf::from("announcement-a.json"),
                PathBuf::from("announcement-b.json")
            ]
        );
    }

    #[test]
    fn catalog_snapshot_announcement_args_require_marker() {
        let error = split_catalog_snapshot_announcement_args(vec![
            "entry-a.json".to_string(),
            "announcement-a.json".to_string(),
        ])
        .unwrap_err();

        assert!(error.contains("--announcements"));
    }
}
