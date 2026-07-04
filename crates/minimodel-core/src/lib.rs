mod manifest;
mod sha256;
mod slm;

use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub use manifest::Manifest;
pub use slm::SlmReport;

use sha256::{bytes_to_hex, normalize_sha256, parse_sha256_hex, sha256, sha256_prefixed};

const MINIMODEL_MANIFEST_SCHEMA_CHECKSUM: &str =
    "sha256:05d9dbd4f31ba6affeb9b4b70672636c296f9d584056f7add9ccc0fc02153cf4";
const DEFAULT_PEER_CHUNK_SIZE: usize = 1_048_576;
const PEER_STATUS_ROUTE: &str = "minimodel.peer_status.v0.json";
const PEER_PIECE_SET_ROUTE: &str = "minimodel.peer_piece_set.v0";
const PEER_COMPANION_STATE_ROUTE: &str = "minimodel.peer_companion_state.v0.json";

#[derive(Debug, Clone)]
pub struct ArtifactVerification {
    pub manifest_sha256: String,
    pub manifest_model_id: String,
    pub artifact_byte_count: u64,
    pub artifact_sha256: String,
    pub evidence_status: String,
    pub slm: SlmReport,
}

#[derive(Debug, Clone)]
pub struct PeerPieceExport {
    pub piece_set_path: String,
    pub piece_count: usize,
    pub manifest_sha256: String,
    pub artifact_sha256: String,
    pub merkle_root_sha256: String,
}

#[derive(Debug, Clone)]
pub struct PeerPieceFetch {
    pub piece_set_path: String,
    pub piece_set_sha256: String,
    pub piece_count: usize,
    pub manifest_sha256: String,
    pub artifact_sha256: String,
    pub merkle_root_sha256: String,
    pub peer_base_url: String,
    pub companion_state_url: String,
    pub companion_state_status: String,
}

#[derive(Debug, Clone)]
pub struct PeerPieceServe {
    pub piece_set_path: String,
    pub piece_set_sha256: String,
    pub model_id: String,
    pub artifact_byte_count: u64,
    pub piece_count: usize,
    pub manifest_sha256: String,
    pub artifact_sha256: String,
    pub merkle_root_sha256: String,
}

#[derive(Debug, Clone)]
pub struct CatalogEntryReport {
    pub catalog_entry_path: String,
    pub model_id: String,
    pub artifact_sha256: String,
    pub peer_offer_sha256: String,
}

#[derive(Debug, Clone)]
pub struct CatalogSnapshotReport {
    pub catalog_path: String,
    pub entry_count: usize,
    pub p2p_ready_count: usize,
    pub peer_source_count: usize,
}

#[derive(Debug, Clone)]
pub struct CatalogSnapshotInspectionReport {
    pub catalog_path: String,
    pub entry_count: usize,
    pub p2p_ready_count: usize,
    pub peer_source_count: usize,
    pub announcement_route_count: usize,
    pub catalog_sha256: String,
}

#[derive(Debug, Clone)]
pub struct CatalogSnapshotWithAnnouncementsReport {
    pub catalog_path: String,
    pub entry_count: usize,
    pub announcement_count: usize,
    pub applied_announcement_count: usize,
    pub p2p_ready_count: usize,
    pub peer_source_count: usize,
}

#[derive(Debug, Clone)]
pub struct PeerSourceAnnouncementFeedReport {
    pub feed_path: String,
    pub announcement_count: usize,
    pub model_count: usize,
    pub feed_sha256: String,
}

#[derive(Debug, Clone)]
pub struct CatalogSnapshotWithAnnouncementFeedReport {
    pub catalog_path: String,
    pub entry_count: usize,
    pub announcement_count: usize,
    pub announcement_model_count: usize,
    pub announcement_feed_route: String,
    pub announcement_feed_sha256: String,
    pub p2p_ready_count: usize,
    pub peer_source_count: usize,
}

#[derive(Debug, Clone)]
pub struct CatalogPeerSourceAttachment {
    pub catalog_entry_path: String,
    pub output_path: String,
    pub model_id: String,
    pub peer_source_count: usize,
    pub peer_base_url: String,
}

#[derive(Debug, Clone)]
pub struct PeerSourceAnnouncementReport {
    pub catalog_entry_path: String,
    pub announcement_path: String,
    pub model_id: String,
    pub artifact_sha256: String,
    pub peer_offer_sha256: String,
    pub peer_base_url: String,
}

#[derive(Debug, Clone)]
pub struct PeerSourceAnnouncementMerge {
    pub catalog_entry_path: String,
    pub output_path: String,
    pub model_id: String,
    pub applied_announcement_count: usize,
    pub peer_source_count: usize,
}

#[derive(Debug, Clone)]
pub struct PeerImportRequestExecution {
    pub request_path: String,
    pub peer_base_url: String,
    pub companion_state_url: String,
    pub companion_state_status: String,
    pub piece_dir_path: String,
    pub piece_set_sha256: String,
    pub output_artifact_path: String,
    pub receipt_path: String,
    pub piece_count: usize,
    pub manifest_sha256: String,
    pub artifact_sha256: String,
    pub merkle_root_sha256: String,
}

#[derive(Debug, Clone)]
pub struct PeerImportRequestInspection {
    pub request_path: String,
    pub request_sha256: String,
    pub manifest_sha256: String,
    pub model_id: String,
    pub artifact_byte_count: u64,
    pub artifact_sha256: String,
    pub expected_peer_base_url: String,
    pub expected_piece_dir: String,
    pub assembled_output_slm: String,
    pub metadata_routes_status: String,
    pub peer_status_route: String,
    pub peer_piece_set_route: String,
    pub peer_companion_state_route: String,
    pub receiver_validation_contract_status: String,
    pub check_peer_status_before_piece_fetch: bool,
    pub preflight_fetches_piece_set_before_pieces: bool,
    pub validate_advertised_companion_state_before_piece_fetch: bool,
    pub import_receipt_required_before_browser_load: bool,
}

#[derive(Debug, Clone)]
pub struct ImportReceiptInspection {
    pub receipt_path: String,
    pub receipt_sha256: String,
    pub manifest_sha256: String,
    pub model_id: String,
    pub artifact_path: String,
    pub artifact_byte_count: u64,
    pub artifact_sha256: String,
    pub slm_model_shape: String,
    pub slm_quantization: String,
    pub peer_import: bool,
    pub peer_base_url: Option<String>,
    pub companion_state_status: Option<String>,
    pub piece_count: Option<usize>,
    pub merkle_root_sha256: Option<String>,
    pub peer_piece_bytes_fetched: bool,
    pub artifact_assembled_from_peer_pieces: bool,
}

#[derive(Debug, Clone)]
pub struct PeerImportReceiptInspection {
    pub receipt_path: String,
    pub receipt_sha256: String,
    pub request_path: String,
    pub request_sha256: String,
    pub preflight_path: String,
    pub preflight_sha256: String,
    pub manifest_sha256: String,
    pub model_id: String,
    pub artifact_path: String,
    pub artifact_sha256: String,
    pub peer_base_url: String,
    pub companion_state_url: String,
    pub companion_state_status: String,
    pub piece_set_sha256: String,
    pub piece_count: usize,
    pub merkle_root_sha256: String,
    pub peer_piece_set_metadata_fetched: bool,
    pub peer_companion_state_metadata_fetched: bool,
    pub peer_piece_bytes_fetched: bool,
    pub artifact_assembled_from_peer_pieces: bool,
    pub project_server_serves_model_bytes: bool,
    pub project_server_runs_model_bytes: bool,
    pub project_server_proxies_huggingface_bytes: bool,
}

#[derive(Debug, Clone)]
pub struct PeerImportPreflight {
    pub request_path: String,
    pub preflight_path: String,
    pub peer_base_url: String,
    pub piece_set_url: String,
    pub piece_set_sha256: String,
    pub companion_state_url: String,
    pub companion_state_status: String,
    pub metadata_routes_status: String,
    pub peer_status_route: String,
    pub peer_piece_set_route: String,
    pub peer_companion_state_route: String,
    pub receiver_validation_contract_status: String,
    pub check_peer_status_before_piece_fetch: bool,
    pub preflight_fetches_piece_set_before_pieces: bool,
    pub validate_advertised_companion_state_before_piece_fetch: bool,
    pub import_receipt_required_before_browser_load: bool,
    pub piece_count: usize,
    pub manifest_sha256: String,
    pub artifact_sha256: String,
    pub merkle_root_sha256: String,
}

#[derive(Debug, Clone)]
pub struct PeerImportPreflightInspection {
    pub preflight_path: String,
    pub preflight_sha256: String,
    pub request_path: String,
    pub request_sha256: String,
    pub manifest_sha256: String,
    pub model_id: String,
    pub peer_base_url: String,
    pub piece_set_url: String,
    pub piece_set_sha256: String,
    pub companion_state_url: String,
    pub companion_state_status: String,
    pub metadata_routes_status: String,
    pub peer_status_route: String,
    pub peer_piece_set_route: String,
    pub peer_companion_state_route: String,
    pub receiver_validation_contract_status: String,
    pub check_peer_status_before_piece_fetch: bool,
    pub preflight_fetches_piece_set_before_pieces: bool,
    pub validate_advertised_companion_state_before_piece_fetch: bool,
    pub import_receipt_required_before_browser_load: bool,
    pub peer_piece_set_metadata_fetched: bool,
    pub peer_companion_state_metadata_fetched: bool,
    pub peer_piece_bytes_fetched: bool,
    pub artifact_bytes_fetched: bool,
    pub project_server_serves_model_bytes: bool,
    pub project_server_runs_model_bytes: bool,
    pub project_server_proxies_huggingface_bytes: bool,
    pub piece_count: usize,
    pub artifact_sha256: String,
    pub merkle_root_sha256: String,
}

/// Summary returned after a metadata-only status check against a running direct peer.
#[derive(Debug, Clone)]
pub struct PeerStatusCheck {
    pub report_path: String,
    pub peer_base_url: String,
    pub status_url: String,
    pub companion_state_route: String,
    pub model_id: String,
    pub artifact_byte_count: u64,
    pub piece_count: usize,
    pub piece_set_sha256: String,
    pub manifest_sha256: String,
    pub artifact_sha256: String,
    pub merkle_root_sha256: String,
}

#[derive(Debug, Clone)]
pub struct PeerSharePreparation {
    pub manifest_path: String,
    pub peer_offer_path: String,
    pub piece_dir_path: String,
    pub catalog_entry_path: String,
    pub model_id: String,
    pub artifact_sha256: String,
    pub piece_count: usize,
    pub peer_source_count: usize,
}

/// Summary returned after writing a local companion-readable share state file.
#[derive(Debug, Clone)]
pub struct PeerCompanionStateReport {
    pub state_path: String,
    pub model_id: String,
    pub peer_base_url: String,
    pub piece_count: usize,
    pub manifest_sha256: String,
    pub artifact_sha256: String,
    pub merkle_root_sha256: String,
}

/// Summary returned after validating a companion state before serving.
#[derive(Debug, Clone)]
pub struct PeerCompanionServe {
    pub state_path: String,
    pub state_route: String,
    pub piece_set_path: String,
    pub piece_set_sha256: String,
    pub model_id: String,
    pub peer_base_url: String,
    pub artifact_byte_count: u64,
    pub piece_count: usize,
    pub manifest_sha256: String,
    pub artifact_sha256: String,
    pub merkle_root_sha256: String,
}

/// Summary returned after writing or validating a metadata-only peer contact card.
#[derive(Debug, Clone)]
pub struct PeerContactCardReport {
    pub card_path: String,
    pub peer_label: String,
    pub peer_base_url: String,
}

/// Summary returned after validating a TinyRustLM local share metadata bundle.
#[derive(Debug, Clone)]
pub struct ShareMetadataReport {
    pub bundle_path: String,
    pub model_id: String,
    pub artifact_file_name: String,
    pub artifact_byte_count: usize,
    pub artifact_sha256: String,
    pub chunk_count: usize,
    pub merkle_root_sha256: String,
    pub peer_transfer_status: String,
    pub peer_transfer_implementation: String,
    pub share_kit_available: bool,
    pub network_transfer_available: bool,
}

#[derive(Debug, Clone)]
struct PeerSourceAnnouncement {
    model_id: String,
    artifact_byte_count: usize,
    artifact_sha256: String,
    peer_offer_route: String,
    peer_offer_chunk_count: usize,
    peer_offer_sha256: String,
    peer_offer_merkle_root_sha256: String,
    peer_base_url: String,
    peer_source: Value,
}

#[derive(Debug, Clone)]
struct PeerCompanionMetadataCheck {
    url: String,
    status: String,
}

#[derive(Debug, Clone)]
struct PeerImportReceiptTransfer {
    request_path: String,
    peer_base_url: String,
    piece_dir_path: String,
    piece_set_path: String,
    piece_set_sha256: String,
    companion_state_url: String,
    companion_state_status: String,
    piece_count: usize,
    merkle_root_sha256: String,
}

pub fn verify_manifest_path(path: &Path) -> Result<Manifest, String> {
    Ok(read_valid_manifest_path(path)?.manifest)
}

pub fn verify_artifact_path(
    manifest_path: &Path,
    artifact_path: &Path,
) -> Result<ArtifactVerification, String> {
    let bundle = read_valid_manifest_path(manifest_path)?;
    let artifact_bytes =
        fs::read(artifact_path).map_err(|error| format!("failed to read artifact: {error}"))?;
    verify_artifact_bytes(&bundle.manifest, &bundle.bytes, &artifact_bytes)
}

pub fn verify_artifact_bytes(
    manifest: &Manifest,
    manifest_bytes: &[u8],
    artifact_bytes: &[u8],
) -> Result<ArtifactVerification, String> {
    let expected_byte_count = manifest
        .require("artifact.byte_count")?
        .parse::<u64>()
        .map_err(|_| "artifact.byte_count must be a decimal integer".to_string())?;
    if artifact_bytes.len() as u64 != expected_byte_count {
        return Err(format!(
            "artifact byte count mismatch: manifest expected {expected_byte_count}, file had {}",
            artifact_bytes.len()
        ));
    }
    let expected_artifact_sha = normalize_sha256(manifest.require("artifact.sha256")?)?;
    let actual_artifact_sha = sha256_prefixed(artifact_bytes);
    if actual_artifact_sha != expected_artifact_sha {
        return Err(format!(
            "artifact SHA-256 mismatch: manifest expected {expected_artifact_sha}, file had {actual_artifact_sha}"
        ));
    }
    verify_chunks(manifest, artifact_bytes)?;
    let slm = slm::validate_slm_bytes(artifact_bytes)?;
    compare_slm_manifest(manifest, &slm)?;
    Ok(ArtifactVerification {
        manifest_sha256: sha256_prefixed(manifest_bytes),
        manifest_model_id: manifest.require("model.id")?.to_string(),
        artifact_byte_count: artifact_bytes.len() as u64,
        artifact_sha256: actual_artifact_sha,
        evidence_status: manifest.require("evidence.admission.status")?.to_string(),
        slm,
    })
}

pub fn write_import_receipt_path(
    manifest_path: &Path,
    artifact_path: &Path,
    receipt_path: &Path,
) -> Result<ArtifactVerification, String> {
    let report = verify_artifact_path(manifest_path, artifact_path)?;
    let receipt = import_receipt_text(&report, artifact_path)?;
    ensure_parent_dir(receipt_path, "receipt")?;
    fs::write(receipt_path, receipt)
        .map_err(|error| format!("failed to write import receipt: {error}"))?;
    Ok(report)
}

fn write_peer_import_receipt_path(
    manifest_path: &Path,
    artifact_path: &Path,
    receipt_path: &Path,
    transfer: &PeerImportReceiptTransfer,
) -> Result<ArtifactVerification, String> {
    let report = verify_artifact_path(manifest_path, artifact_path)?;
    let receipt = peer_import_receipt_text(&report, artifact_path, transfer)?;
    ensure_parent_dir(receipt_path, "peer import receipt")?;
    fs::write(receipt_path, receipt)
        .map_err(|error| format!("failed to write peer import receipt: {error}"))?;
    Ok(report)
}

pub fn export_peer_pieces_path(
    manifest_path: &Path,
    artifact_path: &Path,
    output_dir: &Path,
) -> Result<PeerPieceExport, String> {
    let bundle = read_valid_manifest_path(manifest_path)?;
    require_fixed_chunk_mode(&bundle.manifest)?;
    let artifact_bytes =
        fs::read(artifact_path).map_err(|error| format!("failed to read artifact: {error}"))?;
    let report = verify_artifact_bytes(&bundle.manifest, &bundle.bytes, &artifact_bytes)?;
    create_empty_dir(output_dir)?;
    let chunk_size = parse_usize(bundle.manifest.require("chunks.size")?, "chunks.size")?;
    let chunks = chunk_proofs(&artifact_bytes, chunk_size)?;
    let piece_set_text = peer_piece_set_text(&bundle.manifest, &report, &chunks)?;
    for (index, chunk) in chunks.iter().enumerate() {
        let start = chunk.offset;
        let end = start
            .checked_add(chunk.byte_count)
            .ok_or_else(|| "peer piece range overflow".to_string())?;
        let bytes = artifact_bytes
            .get(start..end)
            .ok_or_else(|| "peer piece range escapes artifact".to_string())?;
        fs::write(output_dir.join(piece_file_name(index)), bytes)
            .map_err(|error| format!("failed to write peer piece {index}: {error}"))?;
    }
    let piece_set_path = output_dir.join("minimodel.peer_piece_set.v0");
    fs::write(&piece_set_path, piece_set_text)
        .map_err(|error| format!("failed to write peer piece set: {error}"))?;
    Ok(PeerPieceExport {
        piece_set_path: piece_set_path.display().to_string(),
        piece_count: chunks.len(),
        manifest_sha256: report.manifest_sha256,
        artifact_sha256: report.artifact_sha256,
        merkle_root_sha256: merkle_root_sha256(&chunks)?,
    })
}

pub fn assemble_peer_pieces_path(
    manifest_path: &Path,
    piece_dir: &Path,
    output_artifact_path: &Path,
) -> Result<ArtifactVerification, String> {
    if output_artifact_path.exists() {
        return Err("output artifact path already exists".to_string());
    }
    let bundle = read_valid_manifest_path(manifest_path)?;
    require_fixed_chunk_mode(&bundle.manifest)?;
    let piece_set_path = piece_dir.join("minimodel.peer_piece_set.v0");
    let piece_set_text = fs::read_to_string(&piece_set_path)
        .map_err(|error| format!("failed to read peer piece set: {error}"))?;
    let piece_set = Manifest::parse(&piece_set_text)?;
    validate_piece_set(&bundle.manifest, &bundle.bytes, &piece_set)?;
    ensure_parent_dir(output_artifact_path, "output")?;
    let temp_path = temp_output_path(output_artifact_path)?;
    if temp_path.exists() {
        return Err(format!(
            "temporary output path already exists: {}",
            temp_path.display()
        ));
    }
    let assemble_result = assemble_peer_pieces_to_temp(&piece_set, piece_dir, &temp_path)
        .and_then(|_| verify_artifact_path(manifest_path, &temp_path));
    match assemble_result {
        Ok(report) => {
            fs::rename(&temp_path, output_artifact_path)
                .map_err(|error| format!("failed to move verified artifact into place: {error}"))?;
            Ok(report)
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(error)
        }
    }
}

pub fn fetch_peer_pieces_http_path(
    manifest_path: &Path,
    peer_base_url: &str,
    output_dir: &Path,
) -> Result<PeerPieceFetch, String> {
    let base_url = HttpUrl::parse(peer_base_url)?;
    reject_project_server_peer_host(&base_url.host)?;
    if output_dir.exists() {
        return Err("peer piece fetch output directory must not already exist".to_string());
    }
    let bundle = read_valid_manifest_path(manifest_path)?;
    require_fixed_chunk_mode(&bundle.manifest)?;
    let piece_set_url = base_url.join_file("minimodel.peer_piece_set.v0")?;
    let piece_set_bytes = http_get_bytes(&piece_set_url, 1_048_576)?;
    let piece_set_sha256 = sha256_prefixed(&piece_set_bytes);
    let piece_set_text = std::str::from_utf8(&piece_set_bytes)
        .map_err(|_| "peer piece set must be valid UTF-8".to_string())?;
    let piece_set = Manifest::parse(piece_set_text)?;
    validate_piece_set(&bundle.manifest, &bundle.bytes, &piece_set)?;
    let companion = validate_optional_peer_companion_state(
        &base_url,
        &piece_set_url,
        &bundle.manifest,
        &bundle.bytes,
        &piece_set,
        piece_set_text.as_bytes(),
    )?;
    let chunk_count = parse_usize(piece_set.require("chunks.count")?, "chunks.count")?;
    let temp_dir = temp_download_dir(output_dir)?;
    if temp_dir.exists() {
        return Err(format!(
            "temporary peer piece directory already exists: {}",
            temp_dir.display()
        ));
    }
    ensure_parent_dir(output_dir, "peer piece output parent")?;
    let result = fetch_peer_pieces_to_temp(&base_url, &piece_set, chunk_count, &temp_dir)
        .and_then(|_| {
            fs::write(
                temp_dir.join("minimodel.peer_piece_set.v0"),
                piece_set_text.as_bytes(),
            )
            .map_err(|error| format!("failed to write peer piece set: {error}"))
        })
        .and_then(|_| {
            fs::rename(&temp_dir, output_dir)
                .map_err(|error| format!("failed to move fetched peer pieces into place: {error}"))
        });
    match result {
        Ok(()) => Ok(PeerPieceFetch {
            piece_set_path: output_dir
                .join("minimodel.peer_piece_set.v0")
                .display()
                .to_string(),
            piece_set_sha256,
            piece_count: chunk_count,
            manifest_sha256: sha256_prefixed(&bundle.bytes),
            artifact_sha256: normalize_sha256(bundle.manifest.require("artifact.sha256")?)?,
            merkle_root_sha256: normalize_sha256(
                bundle.manifest.require("chunks.merkle_root_sha256")?,
            )?,
            peer_base_url: base_url.to_string(),
            companion_state_url: companion.url,
            companion_state_status: companion.status,
        }),
        Err(error) => {
            let _ = fs::remove_dir_all(&temp_dir);
            Err(error)
        }
    }
}

pub fn execute_peer_import_request_path(
    request_path: &Path,
    manifest_path: &Path,
    output_dir: &Path,
) -> Result<PeerImportRequestExecution, String> {
    let request = read_peer_import_request_path(request_path)?;
    let bundle = read_valid_manifest_path(manifest_path)?;
    validate_peer_import_request_against_manifest(&request, &bundle.manifest)?;
    create_empty_dir(output_dir)?;
    let piece_dir = output_dir.join(&request.expected_piece_dir);
    let output_artifact_path = output_dir.join(&request.assembled_output_slm);
    let receipt_path = output_dir.join(receipt_file_name(&request.assembled_output_slm)?);
    let fetch =
        fetch_peer_pieces_http_path(manifest_path, &request.expected_peer_base_url, &piece_dir)?;
    let artifact = assemble_peer_pieces_path(manifest_path, &piece_dir, &output_artifact_path)?;
    write_peer_import_receipt_path(
        manifest_path,
        &output_artifact_path,
        &receipt_path,
        &PeerImportReceiptTransfer {
            request_path: request_path.display().to_string(),
            peer_base_url: fetch.peer_base_url.clone(),
            piece_dir_path: piece_dir.display().to_string(),
            piece_set_path: fetch.piece_set_path.clone(),
            piece_set_sha256: fetch.piece_set_sha256.clone(),
            companion_state_url: fetch.companion_state_url.clone(),
            companion_state_status: fetch.companion_state_status.clone(),
            piece_count: fetch.piece_count,
            merkle_root_sha256: fetch.merkle_root_sha256.clone(),
        },
    )?;
    Ok(PeerImportRequestExecution {
        request_path: request_path.display().to_string(),
        peer_base_url: fetch.peer_base_url,
        companion_state_url: fetch.companion_state_url,
        companion_state_status: fetch.companion_state_status,
        piece_dir_path: piece_dir.display().to_string(),
        piece_set_sha256: fetch.piece_set_sha256,
        output_artifact_path: output_artifact_path.display().to_string(),
        receipt_path: receipt_path.display().to_string(),
        piece_count: fetch.piece_count,
        manifest_sha256: artifact.manifest_sha256,
        artifact_sha256: artifact.artifact_sha256,
        merkle_root_sha256: fetch.merkle_root_sha256,
    })
}

pub fn inspect_peer_import_request_path(
    request_path: &Path,
    manifest_path: &Path,
) -> Result<PeerImportRequestInspection, String> {
    let (request_bytes, request) = read_peer_import_request_bytes_path(request_path)?;
    let bundle = read_valid_manifest_path(manifest_path)?;
    validate_peer_import_request_against_manifest(&request, &bundle.manifest)?;
    let contract = peer_import_request_contract_summary(&request);
    Ok(PeerImportRequestInspection {
        request_path: request_path.display().to_string(),
        request_sha256: sha256_prefixed(&request_bytes),
        manifest_sha256: sha256_prefixed(&bundle.bytes),
        model_id: request.model_id,
        artifact_byte_count: request.artifact_byte_count,
        artifact_sha256: normalize_sha256(&request.artifact_sha256)?,
        expected_peer_base_url: normalize_known_peer_base_url(&request.expected_peer_base_url)?,
        expected_piece_dir: request.expected_piece_dir,
        assembled_output_slm: request.assembled_output_slm,
        metadata_routes_status: contract.metadata_routes_status,
        peer_status_route: contract.peer_status_route,
        peer_piece_set_route: contract.peer_piece_set_route,
        peer_companion_state_route: contract.peer_companion_state_route,
        receiver_validation_contract_status: contract.receiver_validation_contract_status,
        check_peer_status_before_piece_fetch: contract.check_peer_status_before_piece_fetch,
        preflight_fetches_piece_set_before_pieces: contract
            .preflight_fetches_piece_set_before_pieces,
        validate_advertised_companion_state_before_piece_fetch: contract
            .validate_advertised_companion_state_before_piece_fetch,
        import_receipt_required_before_browser_load: contract
            .import_receipt_required_before_browser_load,
    })
}

pub fn inspect_import_receipt_path(
    receipt_path: &Path,
    manifest_path: &Path,
    artifact_path: &Path,
) -> Result<ImportReceiptInspection, String> {
    let receipt_bytes = fs::read(receipt_path)
        .map_err(|error| format!("failed to read import receipt: {error}"))?;
    let receipt_text = std::str::from_utf8(&receipt_bytes)
        .map_err(|_| "import receipt must be valid UTF-8".to_string())?;
    let receipt = Manifest::parse(receipt_text)
        .map_err(|error| format!("failed to parse import receipt: {error}"))?;
    let bundle = read_valid_manifest_path(manifest_path)?;
    let artifact_bytes =
        fs::read(artifact_path).map_err(|error| format!("failed to read artifact: {error}"))?;
    let artifact = verify_artifact_bytes(&bundle.manifest, &bundle.bytes, &artifact_bytes)?;
    let peer =
        validate_import_receipt_fields(&receipt, &bundle.manifest, &artifact, artifact_path)?;

    Ok(ImportReceiptInspection {
        receipt_path: receipt_path.display().to_string(),
        receipt_sha256: sha256_prefixed(&receipt_bytes),
        manifest_sha256: artifact.manifest_sha256,
        model_id: artifact.manifest_model_id,
        artifact_path: artifact_path.display().to_string(),
        artifact_byte_count: artifact.artifact_byte_count,
        artifact_sha256: artifact.artifact_sha256,
        slm_model_shape: artifact.slm.model_shape,
        slm_quantization: artifact.slm.quantization,
        peer_import: peer.peer_import,
        peer_base_url: peer.peer_base_url,
        companion_state_status: peer.companion_state_status,
        piece_count: peer.piece_count,
        merkle_root_sha256: peer.merkle_root_sha256,
        peer_piece_bytes_fetched: peer.peer_piece_bytes_fetched,
        artifact_assembled_from_peer_pieces: peer.artifact_assembled_from_peer_pieces,
    })
}

pub fn inspect_peer_import_receipt_path(
    receipt_path: &Path,
    request_path: &Path,
    preflight_path: &Path,
    manifest_path: &Path,
    artifact_path: &Path,
) -> Result<PeerImportReceiptInspection, String> {
    let receipt_bytes = fs::read(receipt_path)
        .map_err(|error| format!("failed to read import receipt: {error}"))?;
    let receipt_text = std::str::from_utf8(&receipt_bytes)
        .map_err(|_| "import receipt must be valid UTF-8".to_string())?;
    let receipt = Manifest::parse(receipt_text)
        .map_err(|error| format!("failed to parse import receipt: {error}"))?;
    let receipt_created_utc = receipt_required(&receipt, "receipt.created_utc")?;
    let receipt_created_seconds =
        parse_utc_timestamp_seconds(receipt_created_utc, "import receipt receipt.created_utc")?;
    let preflight_bytes = fs::read(preflight_path)
        .map_err(|error| format!("failed to read peer import preflight: {error}"))?;
    let preflight_text = std::str::from_utf8(&preflight_bytes)
        .map_err(|_| "peer import preflight must be valid UTF-8".to_string())?;
    let preflight = Manifest::parse(preflight_text)
        .map_err(|error| format!("failed to parse peer import preflight: {error}"))?;
    let preflight_created_utc = preflight_required(&preflight, "preflight.created_utc")?;
    let preflight_created_seconds = parse_utc_timestamp_seconds(
        preflight_created_utc,
        "peer import preflight preflight.created_utc",
    )?;
    let receipt_report = inspect_import_receipt_path(receipt_path, manifest_path, artifact_path)?;
    if !receipt_report.peer_import {
        return Err("peer import receipt chain requires a peer-import receipt".to_string());
    }
    let preflight_report =
        inspect_peer_import_preflight_path(preflight_path, request_path, manifest_path)?;
    if !preflight_report.import_receipt_required_before_browser_load {
        return Err(
            "peer import receipt chain requires import_receipt_required_before_browser_load=true"
                .to_string(),
        );
    }
    if receipt_created_seconds < preflight_created_seconds {
        return Err(format!(
            "peer import receipt chain receipt.created_utc must be greater than or equal to preflight.created_utc: receipt {receipt_created_utc}, preflight {preflight_created_utc}"
        ));
    }

    require_receipt_path_hint(
        &receipt,
        "peer_import.request_path_hint",
        request_path,
        "peer import request",
    )?;
    require_receipt_equal(&receipt, "peer.base_url", &preflight_report.peer_base_url)?;
    require_receipt_equal(
        &receipt,
        "peer.companion_state_url",
        &preflight_report.companion_state_url,
    )?;
    require_receipt_equal(
        &receipt,
        "peer.companion_state_status",
        &preflight_report.companion_state_status,
    )?;
    require_receipt_sha_equal(
        &receipt,
        "peer.piece_set_sha256",
        &preflight_report.piece_set_sha256,
    )?;
    require_receipt_sha_equal(
        &receipt,
        "artifact.sha256",
        &preflight_report.artifact_sha256,
    )?;
    require_receipt_equal(
        &receipt,
        "chunks.count",
        &preflight_report.piece_count.to_string(),
    )?;
    require_receipt_sha_equal(
        &receipt,
        "chunks.merkle_root_sha256",
        &preflight_report.merkle_root_sha256,
    )?;

    let peer_piece_set_metadata_fetched =
        require_receipt_bool(&receipt, "peer_piece_set_metadata_fetched", true)?;
    let peer_companion_state_metadata_fetched = require_receipt_bool(
        &receipt,
        "peer_companion_state_metadata_fetched",
        preflight_report.peer_companion_state_metadata_fetched,
    )?;
    let peer_piece_bytes_fetched =
        require_receipt_bool(&receipt, "peer_piece_bytes_fetched", true)?;
    let artifact_assembled_from_peer_pieces =
        require_receipt_bool(&receipt, "artifact.assembled_from_peer_pieces", true)?;
    let project_server_serves_model_bytes =
        require_receipt_bool(&receipt, "project_server_serves_model_bytes", false)?;
    let project_server_runs_model_bytes =
        require_receipt_bool(&receipt, "project_server_runs_model_bytes", false)?;
    let project_server_proxies_huggingface_bytes =
        require_receipt_bool(&receipt, "project_server_proxies_huggingface_bytes", false)?;

    Ok(PeerImportReceiptInspection {
        receipt_path: receipt_report.receipt_path,
        receipt_sha256: receipt_report.receipt_sha256,
        request_path: preflight_report.request_path,
        request_sha256: preflight_report.request_sha256,
        preflight_path: preflight_report.preflight_path,
        preflight_sha256: preflight_report.preflight_sha256,
        manifest_sha256: receipt_report.manifest_sha256,
        model_id: receipt_report.model_id,
        artifact_path: receipt_report.artifact_path,
        artifact_sha256: receipt_report.artifact_sha256,
        peer_base_url: preflight_report.peer_base_url,
        companion_state_url: preflight_report.companion_state_url,
        companion_state_status: preflight_report.companion_state_status,
        piece_set_sha256: preflight_report.piece_set_sha256,
        piece_count: preflight_report.piece_count,
        merkle_root_sha256: preflight_report.merkle_root_sha256,
        peer_piece_set_metadata_fetched,
        peer_companion_state_metadata_fetched,
        peer_piece_bytes_fetched,
        artifact_assembled_from_peer_pieces,
        project_server_serves_model_bytes,
        project_server_runs_model_bytes,
        project_server_proxies_huggingface_bytes,
    })
}

pub fn preflight_peer_import_request_path(
    request_path: &Path,
    manifest_path: &Path,
    preflight_path: &Path,
) -> Result<PeerImportPreflight, String> {
    let request = read_peer_import_request_path(request_path)?;
    let bundle = read_valid_manifest_path(manifest_path)?;
    validate_peer_import_request_against_manifest(&request, &bundle.manifest)?;
    let base_url = HttpUrl::parse(&request.expected_peer_base_url)?;
    reject_project_server_peer_host(&base_url.host)?;
    let piece_set_url = base_url.join_file("minimodel.peer_piece_set.v0")?;
    let piece_set_bytes = http_get_bytes(&piece_set_url, 1_048_576)?;
    let piece_set_sha256 = sha256_prefixed(&piece_set_bytes);
    let piece_set_text = std::str::from_utf8(&piece_set_bytes)
        .map_err(|_| "peer piece set must be valid UTF-8".to_string())?;
    let piece_set = Manifest::parse(piece_set_text)?;
    validate_piece_set(&bundle.manifest, &bundle.bytes, &piece_set)?;
    let companion = validate_optional_peer_companion_state(
        &base_url,
        &piece_set_url,
        &bundle.manifest,
        &bundle.bytes,
        &piece_set,
        piece_set_text.as_bytes(),
    )?;
    let piece_count = parse_usize(piece_set.require("chunks.count")?, "chunks.count")?;
    let contract = peer_import_request_contract_summary(&request);
    let report = PeerImportPreflight {
        request_path: request_path.display().to_string(),
        preflight_path: preflight_path.display().to_string(),
        peer_base_url: base_url.to_string(),
        piece_set_url: piece_set_url.to_string(),
        piece_set_sha256,
        companion_state_url: companion.url,
        companion_state_status: companion.status,
        metadata_routes_status: contract.metadata_routes_status,
        peer_status_route: contract.peer_status_route,
        peer_piece_set_route: contract.peer_piece_set_route,
        peer_companion_state_route: contract.peer_companion_state_route,
        receiver_validation_contract_status: contract.receiver_validation_contract_status,
        check_peer_status_before_piece_fetch: contract.check_peer_status_before_piece_fetch,
        preflight_fetches_piece_set_before_pieces: contract
            .preflight_fetches_piece_set_before_pieces,
        validate_advertised_companion_state_before_piece_fetch: contract
            .validate_advertised_companion_state_before_piece_fetch,
        import_receipt_required_before_browser_load: contract
            .import_receipt_required_before_browser_load,
        piece_count,
        manifest_sha256: sha256_prefixed(&bundle.bytes),
        artifact_sha256: normalize_sha256(bundle.manifest.require("artifact.sha256")?)?,
        merkle_root_sha256: normalize_sha256(
            bundle.manifest.require("chunks.merkle_root_sha256")?,
        )?,
    };
    let text = peer_import_preflight_text(&report, &request, &bundle.manifest)?;
    ensure_parent_dir(preflight_path, "peer import preflight")?;
    fs::write(preflight_path, text)
        .map_err(|error| format!("failed to write peer import preflight: {error}"))?;
    Ok(report)
}

pub fn inspect_peer_import_preflight_path(
    preflight_path: &Path,
    request_path: &Path,
    manifest_path: &Path,
) -> Result<PeerImportPreflightInspection, String> {
    let preflight_bytes = fs::read(preflight_path)
        .map_err(|error| format!("failed to read peer import preflight: {error}"))?;
    let preflight_text = std::str::from_utf8(&preflight_bytes)
        .map_err(|_| "peer import preflight must be valid UTF-8".to_string())?;
    let preflight = Manifest::parse(preflight_text)
        .map_err(|error| format!("failed to parse peer import preflight: {error}"))?;
    let (request_bytes, request) = read_peer_import_request_bytes_path(request_path)?;
    let bundle = read_valid_manifest_path(manifest_path)?;
    validate_peer_import_request_against_manifest(&request, &bundle.manifest)?;
    let contract = peer_import_request_contract_summary(&request);
    let base_url = HttpUrl::parse(&request.expected_peer_base_url)?;
    let peer_base_url = normalize_known_peer_base_url(&base_url.to_string())?;
    let piece_set_url = base_url.join_file(PEER_PIECE_SET_ROUTE)?.to_string();
    let companion_state_url = base_url.join_file(PEER_COMPANION_STATE_ROUTE)?.to_string();
    let manifest_sha256 = sha256_prefixed(&bundle.bytes);
    let model_id = bundle.manifest.require("model.id")?.to_string();
    let artifact_sha256 = normalize_sha256(bundle.manifest.require("artifact.sha256")?)?;
    let merkle_root_sha256 =
        normalize_sha256(bundle.manifest.require("chunks.merkle_root_sha256")?)?;
    let piece_count = parse_usize(bundle.manifest.require("chunks.count")?, "chunks.count")?;

    require_preflight_equal(&preflight, "preflight.version", "0")?;
    require_preflight_equal(
        &preflight,
        "preflight.kind",
        "minimodel.peer_import_preflight",
    )?;
    require_preflight_timestampish(preflight_required(&preflight, "preflight.created_utc")?)?;
    require_preflight_path_hint(
        &preflight,
        "request.path",
        request_path,
        "peer import request",
    )?;
    require_preflight_equal(&preflight, "request.kind", &request.request_kind)?;
    require_preflight_sha_equal(&preflight, "manifest.sha256", &manifest_sha256)?;
    require_preflight_equal(&preflight, "manifest.model_id", &model_id)?;
    require_preflight_equal(&preflight, "peer.base_url", &peer_base_url)?;
    require_preflight_equal(
        &preflight,
        "metadata_routes.status",
        &contract.metadata_routes_status,
    )?;
    require_preflight_equal(
        &preflight,
        "metadata_routes.peer_status",
        &contract.peer_status_route,
    )?;
    require_preflight_equal(
        &preflight,
        "metadata_routes.peer_piece_set",
        &contract.peer_piece_set_route,
    )?;
    require_preflight_equal(
        &preflight,
        "metadata_routes.peer_companion_state",
        &contract.peer_companion_state_route,
    )?;
    require_preflight_equal(
        &preflight,
        "receiver_validation_contract.status",
        &contract.receiver_validation_contract_status,
    )?;
    require_preflight_bool(
        &preflight,
        "receiver_validation.check_peer_status_before_piece_fetch",
        contract.check_peer_status_before_piece_fetch,
    )?;
    require_preflight_bool(
        &preflight,
        "receiver_validation.preflight_fetches_piece_set_before_pieces",
        contract.preflight_fetches_piece_set_before_pieces,
    )?;
    require_preflight_bool(
        &preflight,
        "receiver_validation.validate_advertised_companion_state_before_piece_fetch",
        contract.validate_advertised_companion_state_before_piece_fetch,
    )?;
    require_preflight_bool(
        &preflight,
        "receiver_validation.import_receipt_required_before_browser_load",
        contract.import_receipt_required_before_browser_load,
    )?;
    require_preflight_equal(&preflight, "peer.piece_set_url", &piece_set_url)?;
    require_preflight_equal(&preflight, "peer.companion_state_url", &companion_state_url)?;
    let companion_state_status = preflight_required(&preflight, "peer.companion_state_status")?;
    if !matches!(companion_state_status, "validated" | "not-advertised") {
        return Err(format!(
            "peer import preflight peer.companion_state_status must be validated or not-advertised, got {companion_state_status}"
        ));
    }
    require_preflight_equal(&preflight, "piece_set.kind", PEER_PIECE_SET_ROUTE)?;
    require_preflight_equal(&preflight, "piece_set.validation_status", "passed")?;
    let piece_set_sha256 = normalize_sha256(preflight_required(&preflight, "piece_set.sha256")?)?;
    require_preflight_equal(&preflight, "artifact.kind", &request.artifact_kind)?;
    require_preflight_equal(
        &preflight,
        "artifact.byte_count",
        &request.artifact_byte_count.to_string(),
    )?;
    require_preflight_sha_equal(&preflight, "artifact.sha256", &artifact_sha256)?;
    require_preflight_equal(&preflight, "chunks.mode", "fixed-size-merkle-v0")?;
    require_preflight_equal(&preflight, "chunks.count", &piece_count.to_string())?;
    require_preflight_sha_equal(&preflight, "chunks.merkle_root_sha256", &merkle_root_sha256)?;
    let peer_piece_set_metadata_fetched =
        require_preflight_bool(&preflight, "peer_piece_set_metadata_fetched", true)?;
    let expected_companion_fetched = companion_state_status == "validated";
    let peer_companion_state_metadata_fetched = require_preflight_bool(
        &preflight,
        "peer_companion_state_metadata_fetched",
        expected_companion_fetched,
    )?;
    let peer_piece_bytes_fetched =
        require_preflight_bool(&preflight, "peer_piece_bytes_fetched", false)?;
    let artifact_bytes_fetched =
        require_preflight_bool(&preflight, "artifact_bytes_fetched", false)?;
    let project_server_serves_model_bytes =
        require_preflight_bool(&preflight, "project_server_serves_model_bytes", false)?;
    let project_server_runs_model_bytes =
        require_preflight_bool(&preflight, "project_server_runs_model_bytes", false)?;
    let project_server_proxies_huggingface_bytes = require_preflight_bool(
        &preflight,
        "project_server_proxies_huggingface_bytes",
        false,
    )?;
    require_preflight_equal(
        &preflight,
        "required_next_gate",
        "run-import-peer-request-fetch-pieces-assemble-rust-reverify",
    )?;

    Ok(PeerImportPreflightInspection {
        preflight_path: preflight_path.display().to_string(),
        preflight_sha256: sha256_prefixed(&preflight_bytes),
        request_path: request_path.display().to_string(),
        request_sha256: sha256_prefixed(&request_bytes),
        manifest_sha256,
        model_id,
        peer_base_url,
        piece_set_url,
        piece_set_sha256,
        companion_state_url,
        companion_state_status: companion_state_status.to_string(),
        metadata_routes_status: contract.metadata_routes_status,
        peer_status_route: contract.peer_status_route,
        peer_piece_set_route: contract.peer_piece_set_route,
        peer_companion_state_route: contract.peer_companion_state_route,
        receiver_validation_contract_status: contract.receiver_validation_contract_status,
        check_peer_status_before_piece_fetch: contract.check_peer_status_before_piece_fetch,
        preflight_fetches_piece_set_before_pieces: contract
            .preflight_fetches_piece_set_before_pieces,
        validate_advertised_companion_state_before_piece_fetch: contract
            .validate_advertised_companion_state_before_piece_fetch,
        import_receipt_required_before_browser_load: contract
            .import_receipt_required_before_browser_load,
        peer_piece_set_metadata_fetched,
        peer_companion_state_metadata_fetched,
        peer_piece_bytes_fetched,
        artifact_bytes_fetched,
        project_server_serves_model_bytes,
        project_server_runs_model_bytes,
        project_server_proxies_huggingface_bytes,
        piece_count,
        artifact_sha256,
        merkle_root_sha256,
    })
}

pub fn check_peer_status_path(
    manifest_path: &Path,
    peer_base_url: &str,
    output_json: &Path,
) -> Result<PeerStatusCheck, String> {
    let base_url = HttpUrl::parse(peer_base_url)?;
    let normalized_peer_base_url = normalize_known_peer_base_url(&base_url.to_string())?;
    let status_url = base_url.join_file(PEER_STATUS_ROUTE)?;
    let status_url_text = status_url.to_string();
    let bundle = read_valid_manifest_path(manifest_path)?;
    let expected = expected_peer_status_share(
        &bundle.manifest,
        &bundle.bytes,
        base_url
            .join_file("minimodel.peer_piece_set.v0")?
            .to_string(),
    )?;
    let status_bytes = http_get_bytes(&status_url, 32 * 1024)
        .map_err(|error| format!("failed to fetch peer status metadata: {error}"))?;
    let status_value: Value = serde_json::from_slice(&status_bytes)
        .map_err(|error| format!("failed to parse peer status JSON: {error}"))?;
    let companion_state_route =
        validate_peer_status_value(&status_value, &expected, &normalized_peer_base_url)?;
    let report_value = peer_status_check_value(
        &expected,
        &normalized_peer_base_url,
        &status_url_text,
        &companion_state_route,
    )?;
    let report_text = serde_json::to_string_pretty(&report_value)
        .map_err(|error| format!("failed to encode peer status check JSON: {error}"))?
        + "\n";
    ensure_parent_dir(output_json, "peer status check")?;
    fs::write(output_json, report_text)
        .map_err(|error| format!("failed to write peer status check: {error}"))?;
    Ok(PeerStatusCheck {
        report_path: output_json.display().to_string(),
        peer_base_url: normalized_peer_base_url,
        status_url: status_url_text,
        companion_state_route,
        model_id: expected.model_id,
        artifact_byte_count: expected.artifact_byte_count,
        piece_count: expected.piece_count,
        piece_set_sha256: expected.piece_set_sha256,
        manifest_sha256: expected.manifest_sha256,
        artifact_sha256: expected.artifact_sha256,
        merkle_root_sha256: expected.merkle_root_sha256,
    })
}

pub fn inspect_peer_piece_dir_path(
    manifest_path: &Path,
    piece_dir: &Path,
) -> Result<PeerPieceServe, String> {
    let bundle = read_valid_manifest_path(manifest_path)?;
    require_fixed_chunk_mode(&bundle.manifest)?;
    let artifact_byte_count = bundle
        .manifest
        .require("artifact.byte_count")?
        .parse::<u64>()
        .map_err(|_| "artifact.byte_count must be a decimal integer".to_string())?;
    let piece_set_path = piece_dir.join("minimodel.peer_piece_set.v0");
    let piece_set_bytes = fs::read(&piece_set_path)
        .map_err(|error| format!("failed to read peer piece set: {error}"))?;
    let piece_set_text = std::str::from_utf8(&piece_set_bytes)
        .map_err(|_| "peer piece set must be valid UTF-8".to_string())?;
    let piece_set = Manifest::parse(&piece_set_text)?;
    validate_piece_set(&bundle.manifest, &bundle.bytes, &piece_set)?;
    let chunk_count = validate_peer_piece_files(&piece_set, piece_dir)?;
    Ok(PeerPieceServe {
        piece_set_path: piece_set_path.display().to_string(),
        piece_set_sha256: sha256_prefixed(&piece_set_bytes),
        model_id: bundle.manifest.require("model.id")?.to_string(),
        artifact_byte_count,
        piece_count: chunk_count,
        manifest_sha256: sha256_prefixed(&bundle.bytes),
        artifact_sha256: normalize_sha256(bundle.manifest.require("artifact.sha256")?)?,
        merkle_root_sha256: normalize_sha256(
            bundle.manifest.require("chunks.merkle_root_sha256")?,
        )?,
    })
}

/// Validate a prepared peer-piece directory and write local state for a future companion UI.
pub fn write_peer_companion_state_path(
    manifest_path: &Path,
    piece_dir: &Path,
    output_path: &Path,
    known_peer_base_url: &str,
) -> Result<PeerCompanionStateReport, String> {
    let peer_base_url = normalize_known_peer_base_url(known_peer_base_url)?;
    let bundle = read_valid_manifest_path(manifest_path)?;
    let share = inspect_peer_piece_dir_path(manifest_path, piece_dir)?;
    let piece_set_path = piece_dir.join("minimodel.peer_piece_set.v0");
    let piece_set_bytes = fs::read(&piece_set_path)
        .map_err(|error| format!("failed to read peer piece set for companion state: {error}"))?;
    let model_id = bundle.manifest.require("model.id")?.to_string();
    let manifest_hint = local_file_hint(manifest_path, "manifest")?;
    let piece_dir_hint = local_file_hint(piece_dir, "piece directory")?;
    let state = serde_json::json!({
        "state_kind": "minimodel.peer_companion_state",
        "state_version": 0,
        "created_utc": utc_now()?,
        "model_id": model_id,
        "share_status": "ready-to-serve",
        "transfer_protocol": "direct-http-piece-server-v0",
        "peer_base_url": peer_base_url,
        "consent_status": "operator-reviewed-consenting-peer",
        "manifest": {
            "path_hint": manifest_hint,
            "sha256": share.manifest_sha256,
        },
        "artifact": {
            "sha256": share.artifact_sha256,
        },
        "pieces": {
            "directory_hint": piece_dir_hint,
            "piece_set_path_hint": "minimodel.peer_piece_set.v0",
            "piece_set_sha256": sha256_prefixed(&piece_set_bytes),
            "piece_count": share.piece_count,
            "merkle_root_sha256": share.merkle_root_sha256,
        },
        "commands": {
            "serve_peer_pieces": "minimodel serve-peer-pieces <manifest> <piece-dir> <bind-addr>",
            "serve_peer_companion": "minimodel serve-peer-companion <state-json> <manifest> <piece-dir> <bind-addr>",
            "write_peer_source_announcement": "minimodel write-peer-source-announcement <catalog-entry-json> <announcement-json> <known-peer-base-url>",
        },
        "boundary": {
            "user_run_local_software": true,
            "peer_piece_bytes_in_state": false,
            "project_server_serves_model_bytes": false,
            "project_server_runs_model_bytes": false,
            "project_server_proxies_model_bytes": false,
            "tinyrustlm_import_gate": "user-selected-local-slm-after-rust-receipt",
        }
    });
    let state_text = serde_json::to_string_pretty(&state)
        .map_err(|error| format!("failed to encode companion state JSON: {error}"))?
        + "\n";
    ensure_parent_dir(output_path, "peer companion state")?;
    fs::write(output_path, state_text)
        .map_err(|error| format!("failed to write peer companion state: {error}"))?;
    Ok(PeerCompanionStateReport {
        state_path: output_path.display().to_string(),
        model_id,
        peer_base_url,
        piece_count: share.piece_count,
        manifest_sha256: share.manifest_sha256,
        artifact_sha256: share.artifact_sha256,
        merkle_root_sha256: share.merkle_root_sha256,
    })
}

/// Validate a companion state JSON file against the local manifest and peer-piece directory.
pub fn inspect_peer_companion_state_path(
    state_path: &Path,
    manifest_path: &Path,
    piece_dir: &Path,
) -> Result<PeerCompanionServe, String> {
    let manifest = verify_manifest_path(manifest_path)?;
    let model_id = manifest.require("model.id")?.to_string();
    let share = inspect_peer_piece_dir_path(manifest_path, piece_dir)?;
    let piece_set_path = piece_dir.join("minimodel.peer_piece_set.v0");
    let piece_set_bytes = fs::read(&piece_set_path)
        .map_err(|error| format!("failed to read peer piece set for companion serve: {error}"))?;
    let state_bytes = fs::read(state_path)
        .map_err(|error| format!("failed to read peer companion state: {error}"))?;
    let state: Value = serde_json::from_slice(&state_bytes)
        .map_err(|error| format!("failed to parse peer companion state JSON: {error}"))?;
    let peer_base_url = validate_peer_companion_state_value(
        &state,
        &model_id,
        &share,
        &sha256_prefixed(&piece_set_bytes),
    )?;
    Ok(PeerCompanionServe {
        state_path: state_path.display().to_string(),
        state_route: "minimodel.peer_companion_state.v0.json".to_string(),
        piece_set_path: piece_set_path.display().to_string(),
        piece_set_sha256: sha256_prefixed(&piece_set_bytes),
        model_id,
        peer_base_url,
        artifact_byte_count: share.artifact_byte_count,
        piece_count: share.piece_count,
        manifest_sha256: share.manifest_sha256,
        artifact_sha256: share.artifact_sha256,
        merkle_root_sha256: share.merkle_root_sha256,
    })
}

/// Serve a verified companion state plus the prepared peer-piece directory over local HTTP.
pub fn serve_peer_companion_http_path(
    state_path: &Path,
    manifest_path: &Path,
    piece_dir: &Path,
    bind_addr: &str,
) -> Result<PeerCompanionServe, String> {
    let listener = TcpListener::bind(bind_addr)
        .map_err(|error| format!("failed to bind peer companion server at {bind_addr}: {error}"))?;
    serve_peer_companion_http_listener(state_path, manifest_path, piece_dir, listener, None)
}

/// Write a reusable metadata-only contact card for a consenting direct peer.
pub fn write_peer_contact_card_path(
    output_path: &Path,
    known_peer_base_url: &str,
    peer_label: Option<&str>,
) -> Result<PeerContactCardReport, String> {
    let peer_base_url = normalize_known_peer_base_url(known_peer_base_url)?;
    let peer_label = match peer_label {
        Some(value) if !value.trim().is_empty() => validate_peer_contact_label(value)?,
        _ => default_peer_contact_label(&peer_base_url)?,
    };
    let card = serde_json::json!({
        "card_kind": "minimodel.peer_contact_card",
        "card_version": 0,
        "created_utc": utc_now()?,
        "peer_label": peer_label,
        "peer_base_url": peer_base_url,
        "transfer_protocol": "direct-http-piece-server-v0",
        "contact_status": "operator-reviewed-consenting-peer",
        "intended_use": "known-peer-url-for-minimodel-peer-piece-import",
        "project_server_serves_model_bytes": false,
        "project_server_runs_model_bytes": false,
        "project_server_proxies_model_bytes": false,
        "commands": {
            "serve_peer_pieces": "minimodel serve-peer-pieces <manifest> <piece-dir> <bind-addr>",
            "serve_peer_companion": "minimodel serve-peer-companion <state-json> <manifest> <piece-dir> <bind-addr>",
            "import_peer_request": "minimodel import-peer-request <request-json> <manifest> <output-dir>",
        },
        "boundary": {
            "metadata_only": true,
            "peer_contact_card_embeds_model_bytes": false,
            "project_server_serves_model_bytes": false,
            "project_server_runs_model_bytes": false,
            "project_server_proxies_model_bytes": false,
            "requires_rust_reverify_before_browser_import": true,
        }
    });
    let text = serde_json::to_string_pretty(&card)
        .map_err(|error| format!("failed to encode peer contact card JSON: {error}"))?
        + "\n";
    ensure_parent_dir(output_path, "peer contact card")?;
    fs::write(output_path, text)
        .map_err(|error| format!("failed to write peer contact card: {error}"))?;

    Ok(PeerContactCardReport {
        card_path: output_path.display().to_string(),
        peer_label,
        peer_base_url,
    })
}

/// Validate a peer contact card and return the normalized peer URL it names.
pub fn inspect_peer_contact_card_path(card_path: &Path) -> Result<PeerContactCardReport, String> {
    let label = card_path.display().to_string();
    let text = fs::read_to_string(card_path)
        .map_err(|error| format!("failed to read peer contact card {label}: {error}"))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse peer contact card {label}: {error}"))?;
    let (peer_label, peer_base_url) = validate_peer_contact_card_value(&value, &label)?;
    Ok(PeerContactCardReport {
        card_path: label,
        peer_label,
        peer_base_url,
    })
}

/// Validate a TinyRustLM local share metadata bundle before catalog or peer-share use.
pub fn inspect_share_metadata_path(bundle_path: &Path) -> Result<ShareMetadataReport, String> {
    let label = bundle_path.display().to_string();
    let text = fs::read_to_string(bundle_path)
        .map_err(|error| format!("failed to read share metadata bundle {label}: {error}"))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse share metadata bundle {label}: {error}"))?;
    validate_share_metadata_bundle_value(&value, &label, label.clone())
}

pub fn write_catalog_entry_path(
    manifest_path: &Path,
    peer_offer_path: &Path,
    output_path: &Path,
    source_huggingface_model_id: Option<&str>,
    known_peer_base_url: Option<&str>,
) -> Result<CatalogEntryReport, String> {
    let bundle = read_valid_manifest_path(manifest_path)?;
    require_fixed_chunk_mode(&bundle.manifest)?;
    let peer_offer_bytes = fs::read(peer_offer_path)
        .map_err(|error| format!("failed to read peer offer proof: {error}"))?;
    let peer_offer_text = std::str::from_utf8(&peer_offer_bytes)
        .map_err(|_| "peer offer proof must be valid UTF-8".to_string())?;
    let peer_offer = Manifest::parse(peer_offer_text)?;
    let peer_offer_sha256 = sha256_prefixed(&peer_offer_bytes);
    validate_peer_offer_against_manifest(&bundle.manifest, &peer_offer, &peer_offer_sha256)?;
    let manifest_route = file_name_for_route(manifest_path, "manifest")?;
    let peer_offer_route = file_name_for_route(peer_offer_path, "peer offer")?;
    let source_huggingface_model_id = match source_huggingface_model_id {
        Some(value) if !value.trim().is_empty() => Some(validate_huggingface_model_id(value)?),
        _ => None,
    };
    let known_peer_base_url = match known_peer_base_url {
        Some(value) if !value.trim().is_empty() => Some(normalize_known_peer_base_url(value)?),
        _ => None,
    };
    let catalog_entry = catalog_entry_json(
        &bundle.manifest,
        &peer_offer,
        &manifest_route,
        &peer_offer_route,
        &peer_offer_sha256,
        source_huggingface_model_id.as_deref(),
        known_peer_base_url.as_deref(),
    )?;
    ensure_parent_dir(output_path, "catalog entry")?;
    fs::write(output_path, catalog_entry)
        .map_err(|error| format!("failed to write catalog entry: {error}"))?;
    Ok(CatalogEntryReport {
        catalog_entry_path: output_path.display().to_string(),
        model_id: bundle.manifest.require("model.id")?.to_string(),
        artifact_sha256: normalize_sha256(bundle.manifest.require("artifact.sha256")?)?,
        peer_offer_sha256,
    })
}

pub fn write_catalog_snapshot_path(
    output_path: &Path,
    entry_paths: &[impl AsRef<Path>],
) -> Result<CatalogSnapshotReport, String> {
    if entry_paths.is_empty() {
        return Err("catalog snapshot requires at least one catalog-entry JSON".to_string());
    }
    let mut entries = Vec::with_capacity(entry_paths.len());
    let mut model_ids = BTreeSet::new();
    let mut p2p_ready_count = 0usize;
    let mut peer_source_count = 0usize;

    for entry_path in entry_paths {
        let path = entry_path.as_ref();
        let label = path.display().to_string();
        let text = fs::read_to_string(path)
            .map_err(|error| format!("failed to read catalog entry {label}: {error}"))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse catalog entry {label}: {error}"))?;
        let summary = validate_catalog_entry_value(&value, &label)?;
        if !model_ids.insert(summary.model_id.clone()) {
            return Err(format!(
                "duplicate catalog model_id {} in {label}",
                summary.model_id
            ));
        }
        if summary.peer_source_count > 0 {
            p2p_ready_count += 1;
            peer_source_count += summary.peer_source_count;
        }
        entries.push(value);
    }

    let catalog = catalog_snapshot_value(entries)?;
    let text = serde_json::to_string_pretty(&catalog)
        .map_err(|error| format!("failed to format catalog snapshot JSON: {error}"))?
        + "\n";
    ensure_parent_dir(output_path, "catalog snapshot")?;
    fs::write(output_path, text)
        .map_err(|error| format!("failed to write catalog snapshot: {error}"))?;

    Ok(CatalogSnapshotReport {
        catalog_path: output_path.display().to_string(),
        entry_count: entry_paths.len(),
        p2p_ready_count,
        peer_source_count,
    })
}

pub fn inspect_catalog_snapshot_path(
    catalog_path: &Path,
) -> Result<CatalogSnapshotInspectionReport, String> {
    let label = catalog_path.display().to_string();
    let bytes = fs::read(catalog_path)
        .map_err(|error| format!("failed to read catalog snapshot {label}: {error}"))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| format!("catalog snapshot {label} must be valid UTF-8"))?;
    let value: Value = serde_json::from_str(text)
        .map_err(|error| format!("failed to parse catalog snapshot {label}: {error}"))?;
    let summary = validate_catalog_snapshot_value(&value, &label)?;
    Ok(CatalogSnapshotInspectionReport {
        catalog_path: label,
        entry_count: summary.entry_count,
        p2p_ready_count: summary.p2p_ready_count,
        peer_source_count: summary.peer_source_count,
        announcement_route_count: summary.announcement_route_count,
        catalog_sha256: sha256_prefixed(&bytes),
    })
}

pub fn write_catalog_snapshot_with_announcements_path(
    output_path: &Path,
    entry_paths: &[std::path::PathBuf],
    announcement_paths: &[std::path::PathBuf],
) -> Result<CatalogSnapshotWithAnnouncementsReport, String> {
    if entry_paths.is_empty() {
        return Err("catalog snapshot requires at least one catalog-entry JSON".to_string());
    }
    if announcement_paths.is_empty() {
        return Err(
            "catalog snapshot with announcements requires at least one announcement JSON"
                .to_string(),
        );
    }

    let announcements: Vec<(String, PeerSourceAnnouncement)> = announcement_paths
        .iter()
        .map(|path| read_peer_source_announcement_path(path))
        .collect::<Result<_, _>>()?;
    let mut entry_values = Vec::with_capacity(entry_paths.len());
    let mut model_ids = BTreeSet::new();

    for entry_path in entry_paths {
        let label = entry_path.display().to_string();
        let text = fs::read_to_string(entry_path)
            .map_err(|error| format!("failed to read catalog entry {label}: {error}"))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse catalog entry {label}: {error}"))?;
        let summary = validate_catalog_entry_value(&value, &label)?;
        if !model_ids.insert(summary.model_id.clone()) {
            return Err(format!(
                "duplicate catalog model_id {} in {label}",
                summary.model_id
            ));
        }
        entry_values.push((label, value, summary.model_id));
    }

    for (announcement_label, announcement) in &announcements {
        if !model_ids.contains(&announcement.model_id) {
            return Err(format!(
                "{announcement_label} model_id {} did not match any catalog entry",
                announcement.model_id
            ));
        }
    }

    let mut entries = Vec::with_capacity(entry_values.len());
    let mut p2p_ready_count = 0usize;
    let mut peer_source_count = 0usize;
    let mut applied_announcement_count = 0usize;

    for (label, value, model_id) in entry_values {
        let matching: Vec<(String, PeerSourceAnnouncement)> = announcements
            .iter()
            .filter(|(_, announcement)| announcement.model_id == model_id)
            .cloned()
            .collect();
        let (value, applied_count, final_peer_source_count) = if matching.is_empty() {
            let summary = validate_catalog_entry_value(&value, &label)?;
            (value, 0, summary.peer_source_count)
        } else {
            let output_label = format!("{label} with peer-source announcements");
            let (value, _model_id, applied_count, final_peer_source_count) =
                apply_peer_source_announcements_to_catalog_value(
                    &label,
                    value,
                    &matching,
                    &output_label,
                )?;
            (value, applied_count, final_peer_source_count)
        };
        if final_peer_source_count > 0 {
            p2p_ready_count += 1;
            peer_source_count += final_peer_source_count;
        }
        applied_announcement_count += applied_count;
        entries.push(value);
    }

    let catalog = catalog_snapshot_value(entries)?;
    let text = serde_json::to_string_pretty(&catalog)
        .map_err(|error| format!("failed to format catalog snapshot JSON: {error}"))?
        + "\n";
    ensure_parent_dir(output_path, "catalog snapshot")?;
    fs::write(output_path, text)
        .map_err(|error| format!("failed to write catalog snapshot: {error}"))?;

    Ok(CatalogSnapshotWithAnnouncementsReport {
        catalog_path: output_path.display().to_string(),
        entry_count: entry_paths.len(),
        announcement_count: announcement_paths.len(),
        applied_announcement_count,
        p2p_ready_count,
        peer_source_count,
    })
}

pub fn write_peer_source_announcement_feed_path(
    output_path: &Path,
    announcement_paths: &[std::path::PathBuf],
) -> Result<PeerSourceAnnouncementFeedReport, String> {
    let mut announcement_values = Vec::with_capacity(announcement_paths.len());
    for path in announcement_paths {
        let (_label, value, _announcement) = read_peer_source_announcement_value_path(path)?;
        announcement_values.push(value);
    }

    let mut object = Map::new();
    object.insert(
        "announcement_catalog_kind".to_string(),
        Value::String("minimodel.peer_source_announcement_catalog".to_string()),
    );
    object.insert(
        "announcement_catalog_version".to_string(),
        Value::Number(0.into()),
    );
    object.insert("generated_at".to_string(), Value::String(utc_now()?));
    object.insert(
        "publisher_id".to_string(),
        Value::String("local-user".to_string()),
    );
    object.insert(
        "publisher_key_id".to_string(),
        Value::String("local-user-unsigned".to_string()),
    );
    object.insert(
        "project_server_serves_model_bytes".to_string(),
        Value::Bool(false),
    );
    object.insert(
        "project_server_runs_model_bytes".to_string(),
        Value::Bool(false),
    );
    object.insert(
        "project_server_proxies_model_bytes".to_string(),
        Value::Bool(false),
    );
    object.insert(
        "announcements".to_string(),
        Value::Array(announcement_values),
    );

    let feed = Value::Object(object);
    let announcements =
        validate_peer_source_announcement_feed_value(&feed, &output_path.display().to_string())?;
    let model_count = announcements
        .iter()
        .map(|(_, announcement)| announcement.model_id.clone())
        .collect::<BTreeSet<_>>()
        .len();
    let text = serde_json::to_string_pretty(&feed)
        .map_err(|error| format!("failed to format peer-source announcement feed JSON: {error}"))?
        + "\n";
    let feed_sha256 = sha256_prefixed(text.as_bytes());
    ensure_parent_dir(output_path, "peer-source announcement feed")?;
    fs::write(output_path, text)
        .map_err(|error| format!("failed to write peer-source announcement feed: {error}"))?;

    Ok(PeerSourceAnnouncementFeedReport {
        feed_path: output_path.display().to_string(),
        announcement_count: announcements.len(),
        model_count,
        feed_sha256,
    })
}

pub fn inspect_peer_source_announcement_feed_path(
    feed_json_path: &Path,
) -> Result<PeerSourceAnnouncementFeedReport, String> {
    let label = feed_json_path.display().to_string();
    let text = fs::read_to_string(feed_json_path).map_err(|error| {
        format!("failed to read peer-source announcement feed {label}: {error}")
    })?;
    let value: Value = serde_json::from_str(&text).map_err(|error| {
        format!("failed to parse peer-source announcement feed {label}: {error}")
    })?;
    let announcements = validate_peer_source_announcement_feed_value(&value, &label)?;
    let model_count = announcements
        .iter()
        .map(|(_, announcement)| announcement.model_id.clone())
        .collect::<BTreeSet<_>>()
        .len();

    Ok(PeerSourceAnnouncementFeedReport {
        feed_path: label,
        announcement_count: announcements.len(),
        model_count,
        feed_sha256: sha256_prefixed(text.as_bytes()),
    })
}

pub fn write_catalog_snapshot_with_announcement_feed_path(
    output_path: &Path,
    feed_route: &str,
    feed_json_path: &Path,
    entry_paths: &[std::path::PathBuf],
) -> Result<CatalogSnapshotWithAnnouncementFeedReport, String> {
    if entry_paths.is_empty() {
        return Err("catalog snapshot requires at least one catalog-entry JSON".to_string());
    }
    let feed_route = validate_catalog_announcement_route(feed_route)?;
    let feed_label = feed_json_path.display().to_string();
    let feed_text = fs::read_to_string(feed_json_path).map_err(|error| {
        format!("failed to read peer-source announcement feed {feed_label}: {error}")
    })?;
    let feed_value: Value = serde_json::from_str(&feed_text).map_err(|error| {
        format!("failed to parse peer-source announcement feed {feed_label}: {error}")
    })?;
    let announcements = validate_peer_source_announcement_feed_value(&feed_value, &feed_label)?;
    let announcement_count = announcements.len();
    let announcement_model_ids = announcements
        .iter()
        .map(|(_, announcement)| announcement.model_id.clone())
        .collect::<BTreeSet<_>>();
    let announcement_model_count = announcement_model_ids.len();
    let announcement_feed_sha256 = sha256_prefixed(feed_text.as_bytes());

    let mut entries = Vec::with_capacity(entry_paths.len());
    let mut model_ids = BTreeSet::new();
    let mut p2p_ready_count = 0usize;
    let mut peer_source_count = 0usize;
    for entry_path in entry_paths {
        let label = entry_path.display().to_string();
        let text = fs::read_to_string(entry_path)
            .map_err(|error| format!("failed to read catalog entry {label}: {error}"))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse catalog entry {label}: {error}"))?;
        let summary = validate_catalog_entry_value(&value, &label)?;
        if !model_ids.insert(summary.model_id.clone()) {
            return Err(format!(
                "duplicate catalog model_id {} in {label}",
                summary.model_id
            ));
        }
        if summary.peer_source_count > 0 {
            p2p_ready_count += 1;
            peer_source_count += summary.peer_source_count;
        }
        entries.push(value);
    }

    for model_id in &announcement_model_ids {
        if !model_ids.contains(model_id) {
            return Err(format!(
                "peer-source announcement feed model_id {model_id} did not match any catalog entry"
            ));
        }
    }

    let route_value = catalog_announcement_route_value(&feed_route, &announcement_feed_sha256)?;
    let catalog = catalog_snapshot_value_with_announcement_routes(entries, vec![route_value])?;
    let text = serde_json::to_string_pretty(&catalog)
        .map_err(|error| format!("failed to format catalog snapshot JSON: {error}"))?
        + "\n";
    ensure_parent_dir(output_path, "catalog snapshot")?;
    fs::write(output_path, text)
        .map_err(|error| format!("failed to write catalog snapshot: {error}"))?;

    Ok(CatalogSnapshotWithAnnouncementFeedReport {
        catalog_path: output_path.display().to_string(),
        entry_count: entry_paths.len(),
        announcement_count,
        announcement_model_count,
        announcement_feed_route: feed_route,
        announcement_feed_sha256,
        p2p_ready_count,
        peer_source_count,
    })
}

pub fn add_peer_source_to_catalog_entry_path(
    catalog_entry_path: &Path,
    output_path: &Path,
    known_peer_base_url: &str,
) -> Result<CatalogPeerSourceAttachment, String> {
    let peer_base_url = normalize_known_peer_base_url(known_peer_base_url)?;
    let label = catalog_entry_path.display().to_string();
    let text = fs::read_to_string(catalog_entry_path)
        .map_err(|error| format!("failed to read catalog entry {label}: {error}"))?;
    let mut value: Value = serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse catalog entry {label}: {error}"))?;
    let original_summary = validate_catalog_entry_value(&value, &label)?;
    if original_summary.peer_source_count != 0 {
        return Err(format!(
            "{label} already has {} peer source(s); add-peer-source only upgrades proof-ready catalog entries",
            original_summary.peer_source_count
        ));
    }
    let object = value
        .as_object_mut()
        .ok_or_else(|| format!("{label} catalog entry must be a JSON object"))?;
    object.insert("peer_source_count".to_string(), Value::Number(1.into()));
    object.insert(
        "peer_sources".to_string(),
        serde_json::from_str(&known_peer_sources_json(&peer_base_url))
            .map_err(|error| format!("failed to build peer source metadata: {error}"))?,
    );

    let output_label = output_path.display().to_string();
    let updated_summary = validate_catalog_entry_value(&value, &output_label)?;
    if updated_summary.model_id != original_summary.model_id {
        return Err(format!(
            "{output_label} model_id changed while attaching peer source"
        ));
    }
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| format!("failed to format catalog entry JSON: {error}"))?
        + "\n";
    ensure_parent_dir(output_path, "catalog entry")?;
    fs::write(output_path, text)
        .map_err(|error| format!("failed to write catalog entry: {error}"))?;

    Ok(CatalogPeerSourceAttachment {
        catalog_entry_path: catalog_entry_path.display().to_string(),
        output_path: output_path.display().to_string(),
        model_id: updated_summary.model_id,
        peer_source_count: updated_summary.peer_source_count,
        peer_base_url,
    })
}

pub fn write_peer_source_announcement_path(
    catalog_entry_path: &Path,
    output_path: &Path,
    known_peer_base_url: &str,
) -> Result<PeerSourceAnnouncementReport, String> {
    let peer_base_url = normalize_known_peer_base_url(known_peer_base_url)?;
    let label = catalog_entry_path.display().to_string();
    let text = fs::read_to_string(catalog_entry_path)
        .map_err(|error| format!("failed to read catalog entry {label}: {error}"))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse catalog entry {label}: {error}"))?;
    let summary = validate_catalog_entry_value(&value, &label)?;
    let artifact_byte_count = json_required_usize(&value, "artifact_byte_count", &label)?;
    let artifact_sha256 =
        normalize_sha256(&json_required_string(&value, "artifact_sha256", &label)?)?;
    let peer_offer_sha256 =
        normalize_sha256(&json_required_string(&value, "peer_offer_sha256", &label)?)?;
    let peer_offer_merkle_root_sha256 = normalize_sha256(&json_required_string(
        &value,
        "peer_offer_merkle_root_sha256",
        &label,
    )?)?;
    let peer_offer_route = json_required_string(&value, "peer_offer_route", &label)?;
    let peer_offer_chunk_count = json_required_usize(&value, "peer_offer_chunk_count", &label)?;

    let mut object = Map::new();
    object.insert(
        "announcement_kind".to_string(),
        Value::String("minimodel.peer_source_announcement".to_string()),
    );
    object.insert("announcement_version".to_string(), Value::Number(0.into()));
    object.insert("created_utc".to_string(), Value::String(utc_now()?));
    object.insert(
        "announcement_status".to_string(),
        Value::String("peer-source-available-metadata-only".to_string()),
    );
    object.insert(
        "model_id".to_string(),
        Value::String(summary.model_id.clone()),
    );
    object.insert(
        "artifact_kind".to_string(),
        Value::String("slm".to_string()),
    );
    object.insert(
        "artifact_byte_count".to_string(),
        Value::Number((artifact_byte_count as u64).into()),
    );
    object.insert(
        "artifact_sha256".to_string(),
        Value::String(artifact_sha256.clone()),
    );
    object.insert(
        "peer_offer_route".to_string(),
        Value::String(peer_offer_route),
    );
    object.insert(
        "peer_offer_sha256".to_string(),
        Value::String(peer_offer_sha256.clone()),
    );
    object.insert(
        "peer_offer_merkle_root_sha256".to_string(),
        Value::String(peer_offer_merkle_root_sha256),
    );
    object.insert(
        "peer_offer_chunk_count".to_string(),
        Value::Number((peer_offer_chunk_count as u64).into()),
    );
    object.insert(
        "catalog_entry_sha256".to_string(),
        Value::String(sha256_prefixed(text.as_bytes())),
    );
    object.insert(
        "project_server_serves_model_bytes".to_string(),
        Value::Bool(false),
    );
    object.insert(
        "project_server_runs_model_bytes".to_string(),
        Value::Bool(false),
    );
    object.insert(
        "project_server_proxies_model_bytes".to_string(),
        Value::Bool(false),
    );
    object.insert(
        "peer_source".to_string(),
        known_peer_source_value(&peer_base_url),
    );

    let announcement = Value::Object(object);
    validate_peer_source_announcement_value(&announcement, &output_path.display().to_string())?;
    let text = serde_json::to_string_pretty(&announcement)
        .map_err(|error| format!("failed to format peer source announcement JSON: {error}"))?
        + "\n";
    ensure_parent_dir(output_path, "peer source announcement")?;
    fs::write(output_path, text)
        .map_err(|error| format!("failed to write peer source announcement: {error}"))?;

    Ok(PeerSourceAnnouncementReport {
        catalog_entry_path: catalog_entry_path.display().to_string(),
        announcement_path: output_path.display().to_string(),
        model_id: summary.model_id,
        artifact_sha256,
        peer_offer_sha256,
        peer_base_url,
    })
}

pub fn apply_peer_source_announcements_path(
    catalog_entry_path: &Path,
    output_path: &Path,
    announcement_paths: &[std::path::PathBuf],
) -> Result<PeerSourceAnnouncementMerge, String> {
    if announcement_paths.is_empty() {
        return Err(
            "apply-peer-source-announcements requires at least one announcement".to_string(),
        );
    }
    let label = catalog_entry_path.display().to_string();
    let text = fs::read_to_string(catalog_entry_path)
        .map_err(|error| format!("failed to read catalog entry {label}: {error}"))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse catalog entry {label}: {error}"))?;
    let output_label = output_path.display().to_string();
    let announcements: Vec<(String, PeerSourceAnnouncement)> = announcement_paths
        .iter()
        .map(|path| read_peer_source_announcement_path(path))
        .collect::<Result<_, _>>()?;
    let (value, model_id, applied_count, peer_source_count) =
        apply_peer_source_announcements_to_catalog_value(
            &label,
            value,
            &announcements,
            &output_label,
        )?;
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| format!("failed to format catalog entry JSON: {error}"))?
        + "\n";
    ensure_parent_dir(output_path, "catalog entry")?;
    fs::write(output_path, text)
        .map_err(|error| format!("failed to write catalog entry: {error}"))?;

    Ok(PeerSourceAnnouncementMerge {
        catalog_entry_path: catalog_entry_path.display().to_string(),
        output_path: output_path.display().to_string(),
        model_id,
        applied_announcement_count: applied_count,
        peer_source_count,
    })
}

pub fn prepare_peer_share_path(
    artifact_path: &Path,
    output_dir: &Path,
    model_id: &str,
    source_huggingface_model_id: Option<&str>,
    known_peer_base_url: Option<&str>,
) -> Result<PeerSharePreparation, String> {
    let model_id = validate_minimodel_model_id(model_id)?;
    let source_huggingface_model_id = match source_huggingface_model_id {
        Some(value) if !value.trim().is_empty() => Some(validate_huggingface_model_id(value)?),
        _ => None,
    };
    let known_peer_base_url = match known_peer_base_url {
        Some(value) if !value.trim().is_empty() => Some(normalize_known_peer_base_url(value)?),
        _ => None,
    };
    let artifact_file_name = artifact_file_name_for_share(artifact_path)?;
    let stem = artifact_stem_for_share(&artifact_file_name)?;
    let artifact_bytes =
        fs::read(artifact_path).map_err(|error| format!("failed to read artifact: {error}"))?;
    let slm = slm::validate_slm_bytes(&artifact_bytes)?;
    let chunks = chunk_proofs(&artifact_bytes, DEFAULT_PEER_CHUNK_SIZE)?;
    let peer_offer_path = output_dir.join(format!("{stem}.minimodel-peer-offer"));
    let peer_offer_route = file_name_for_route(&peer_offer_path, "peer offer")?;
    let peer_offer_text = peer_offer_text(&artifact_bytes, &slm, &artifact_file_name, &chunks)?;
    let peer_offer_sha256 = sha256_prefixed(peer_offer_text.as_bytes());
    let manifest_path = output_dir.join(format!("{stem}.minimodel.manifest.v0"));
    let manifest_text = minimodel_manifest_text(
        &model_id,
        &artifact_bytes,
        &slm,
        &chunks,
        &peer_offer_route,
        &peer_offer_sha256,
        source_huggingface_model_id.as_deref(),
    )?;
    let manifest = Manifest::parse(&manifest_text)?;
    manifest.validate()?;
    verify_artifact_bytes(&manifest, manifest_text.as_bytes(), &artifact_bytes)?;

    create_empty_dir(output_dir)?;
    fs::write(&manifest_path, manifest_text)
        .map_err(|error| format!("failed to write MiniModel manifest: {error}"))?;
    fs::write(&peer_offer_path, peer_offer_text)
        .map_err(|error| format!("failed to write peer offer proof: {error}"))?;

    let piece_dir = output_dir.join(format!("{stem}.peer-pieces"));
    let piece_report = export_peer_pieces_path(&manifest_path, artifact_path, &piece_dir)?;
    let catalog_entry_path = output_dir.join(format!("{stem}.catalog-entry.v0.json"));
    let catalog_report = write_catalog_entry_path(
        &manifest_path,
        &peer_offer_path,
        &catalog_entry_path,
        source_huggingface_model_id.as_deref(),
        known_peer_base_url.as_deref(),
    )?;

    Ok(PeerSharePreparation {
        manifest_path: manifest_path.display().to_string(),
        peer_offer_path: peer_offer_path.display().to_string(),
        piece_dir_path: piece_dir.display().to_string(),
        catalog_entry_path: catalog_entry_path.display().to_string(),
        model_id,
        artifact_sha256: catalog_report.artifact_sha256,
        piece_count: piece_report.piece_count,
        peer_source_count: usize::from(known_peer_base_url.is_some()),
    })
}

pub fn serve_peer_pieces_http_path(
    manifest_path: &Path,
    piece_dir: &Path,
    bind_addr: &str,
) -> Result<PeerPieceServe, String> {
    let listener = TcpListener::bind(bind_addr)
        .map_err(|error| format!("failed to bind peer piece server at {bind_addr}: {error}"))?;
    serve_peer_pieces_http_listener(
        manifest_path,
        piece_dir,
        listener,
        None,
        &format!("http://{bind_addr}/"),
    )
}

struct ValidManifest {
    bytes: Vec<u8>,
    manifest: Manifest,
}

#[derive(Debug, Clone)]
struct PeerImportRequest {
    request_kind: String,
    request_version: u64,
    model_id: String,
    artifact_kind: String,
    artifact_byte_count: u64,
    artifact_sha256: String,
    expected_piece_set_kind: String,
    expected_peer_base_url: String,
    expected_piece_dir: String,
    assembled_output_slm: String,
    metadata_routes: Option<PeerImportMetadataRoutes>,
    receiver_validation_contract: Option<PeerImportReceiverValidationContract>,
    project_server_serves_model_bytes: bool,
    project_server_runs_model_bytes: bool,
    project_server_proxies_huggingface_bytes: bool,
    transfer_requires_user_consent_and_local_reverify: bool,
}

#[derive(Debug, Deserialize)]
struct PeerImportRequestFile {
    request_kind: String,
    request_version: u64,
    model: PeerImportRequestModel,
    local_peer_piece_contract: PeerImportPieceContract,
    boundary: PeerImportBoundary,
}

#[derive(Debug, Deserialize)]
struct PeerImportRequestModel {
    model_id: String,
    artifact_kind: String,
    artifact_byte_count: u64,
    artifact_sha256: String,
}

#[derive(Debug, Deserialize)]
struct PeerImportPieceContract {
    expected_piece_set_kind: String,
    expected_peer_base_url: String,
    expected_piece_dir: String,
    assembled_output_slm: String,
    metadata_routes: Option<PeerImportMetadataRoutes>,
    receiver_validation_contract: Option<PeerImportReceiverValidationContract>,
}

#[derive(Debug, Clone, Deserialize)]
struct PeerImportMetadataRoutes {
    peer_status: Option<String>,
    peer_piece_set: Option<String>,
    peer_companion_state: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PeerImportReceiverValidationContract {
    check_peer_status_before_piece_fetch: Option<bool>,
    preflight_fetches_piece_set_before_pieces: Option<bool>,
    validate_advertised_companion_state_before_piece_fetch: Option<bool>,
    import_receipt_required_before_browser_load: Option<bool>,
}

#[derive(Debug, Clone)]
struct PeerImportRequestContractSummary {
    metadata_routes_status: String,
    peer_status_route: String,
    peer_piece_set_route: String,
    peer_companion_state_route: String,
    receiver_validation_contract_status: String,
    check_peer_status_before_piece_fetch: bool,
    preflight_fetches_piece_set_before_pieces: bool,
    validate_advertised_companion_state_before_piece_fetch: bool,
    import_receipt_required_before_browser_load: bool,
}

#[derive(Debug, Deserialize)]
struct PeerImportBoundary {
    project_server_serves_model_bytes: bool,
    project_server_runs_model_bytes: bool,
    project_server_proxies_huggingface_bytes: bool,
    transfer_requires_user_consent_and_local_reverify: bool,
}

impl From<PeerImportRequestFile> for PeerImportRequest {
    fn from(file: PeerImportRequestFile) -> Self {
        Self {
            request_kind: file.request_kind,
            request_version: file.request_version,
            model_id: file.model.model_id,
            artifact_kind: file.model.artifact_kind,
            artifact_byte_count: file.model.artifact_byte_count,
            artifact_sha256: file.model.artifact_sha256,
            expected_piece_set_kind: file.local_peer_piece_contract.expected_piece_set_kind,
            expected_peer_base_url: file.local_peer_piece_contract.expected_peer_base_url,
            expected_piece_dir: file.local_peer_piece_contract.expected_piece_dir,
            assembled_output_slm: file.local_peer_piece_contract.assembled_output_slm,
            metadata_routes: file.local_peer_piece_contract.metadata_routes,
            receiver_validation_contract: file
                .local_peer_piece_contract
                .receiver_validation_contract,
            project_server_serves_model_bytes: file.boundary.project_server_serves_model_bytes,
            project_server_runs_model_bytes: file.boundary.project_server_runs_model_bytes,
            project_server_proxies_huggingface_bytes: file
                .boundary
                .project_server_proxies_huggingface_bytes,
            transfer_requires_user_consent_and_local_reverify: file
                .boundary
                .transfer_requires_user_consent_and_local_reverify,
        }
    }
}

fn read_valid_manifest_path(path: &Path) -> Result<ValidManifest, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read manifest: {error}"))?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| "manifest must be valid UTF-8".to_string())?;
    let manifest = Manifest::parse(text)?;
    manifest.validate()?;
    Ok(ValidManifest { bytes, manifest })
}

fn read_peer_import_request_path(path: &Path) -> Result<PeerImportRequest, String> {
    let (_, request) = read_peer_import_request_bytes_path(path)?;
    Ok(request)
}

fn read_peer_import_request_bytes_path(
    path: &Path,
) -> Result<(Vec<u8>, PeerImportRequest), String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read peer import request: {error}"))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "peer import request must be valid UTF-8".to_string())?;
    let request_file: PeerImportRequestFile = serde_json::from_str(text)
        .map_err(|error| format!("failed to parse peer import request JSON: {error}"))?;
    let request = PeerImportRequest::from(request_file);
    validate_peer_import_request_static_fields(&request)?;
    Ok((bytes, request))
}

fn validate_peer_import_request_static_fields(request: &PeerImportRequest) -> Result<(), String> {
    if request.request_kind != "minimodel.peer_import_request.v0" {
        return Err(format!(
            "peer import request kind mismatch: expected minimodel.peer_import_request.v0, got {}",
            request.request_kind
        ));
    }
    if request.request_version != 0 {
        return Err("peer import request version must be 0".to_string());
    }
    if request.artifact_kind != "slm" {
        return Err(format!(
            "peer import artifact kind mismatch: expected slm, got {}",
            request.artifact_kind
        ));
    }
    if request.expected_piece_set_kind != "minimodel.peer_piece_set.v0" {
        return Err(format!(
            "peer import expected piece set kind mismatch: expected minimodel.peer_piece_set.v0, got {}",
            request.expected_piece_set_kind
        ));
    }
    validate_peer_import_metadata_routes(request.metadata_routes.as_ref())?;
    validate_peer_import_receiver_contract(request.receiver_validation_contract.as_ref())?;
    let peer_url = HttpUrl::parse(&request.expected_peer_base_url)?;
    reject_project_server_peer_host(&peer_url.host)?;
    require_safe_import_name(&request.expected_piece_dir, "expected_piece_dir")?;
    require_safe_import_name(&request.assembled_output_slm, "assembled_output_slm")?;
    if !request.assembled_output_slm.ends_with(".slm") {
        return Err("assembled_output_slm must end with .slm".to_string());
    }
    if request.project_server_serves_model_bytes {
        return Err("peer import request claims project servers serve model bytes".to_string());
    }
    if request.project_server_runs_model_bytes {
        return Err("peer import request claims project servers run model bytes".to_string());
    }
    if request.project_server_proxies_huggingface_bytes {
        return Err(
            "peer import request claims project servers proxy Hugging Face bytes".to_string(),
        );
    }
    if !request.transfer_requires_user_consent_and_local_reverify {
        return Err(
            "peer import request must require user consent and local reverification".to_string(),
        );
    }
    Ok(())
}

fn validate_peer_import_metadata_routes(
    routes: Option<&PeerImportMetadataRoutes>,
) -> Result<(), String> {
    let Some(routes) = routes else {
        return Ok(());
    };
    match routes.peer_status.as_deref() {
        Some("minimodel.peer_status.v0.json") => {}
        Some(route) => {
            return Err(format!(
                "peer import metadata route peer_status must be minimodel.peer_status.v0.json, got {route}"
            ));
        }
        None => {
            return Err(
                "peer import metadata route peer_status must be declared when metadata_routes is present"
                    .to_string(),
            );
        }
    }
    match routes.peer_piece_set.as_deref() {
        Some("minimodel.peer_piece_set.v0") => {}
        Some(route) => {
            return Err(format!(
                "peer import metadata route peer_piece_set must be minimodel.peer_piece_set.v0, got {route}"
            ));
        }
        None => {
            return Err(
                "peer import metadata route peer_piece_set must be declared when metadata_routes is present"
                    .to_string(),
            );
        }
    }
    match routes.peer_companion_state.as_deref() {
        Some("minimodel.peer_companion_state.v0.json") => {}
        Some(route) => {
            return Err(format!(
                "peer import metadata route peer_companion_state must be minimodel.peer_companion_state.v0.json, got {route}"
            ));
        }
        None => {
            return Err(
                "peer import metadata route peer_companion_state must be declared when metadata_routes is present"
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn validate_peer_import_receiver_contract(
    contract: Option<&PeerImportReceiverValidationContract>,
) -> Result<(), String> {
    let Some(contract) = contract else {
        return Ok(());
    };
    let required = [
        (
            "check_peer_status_before_piece_fetch",
            contract.check_peer_status_before_piece_fetch,
        ),
        (
            "preflight_fetches_piece_set_before_pieces",
            contract.preflight_fetches_piece_set_before_pieces,
        ),
        (
            "validate_advertised_companion_state_before_piece_fetch",
            contract.validate_advertised_companion_state_before_piece_fetch,
        ),
        (
            "import_receipt_required_before_browser_load",
            contract.import_receipt_required_before_browser_load,
        ),
    ];
    for (field, value) in required {
        if value != Some(true) {
            return Err(format!(
                "peer import receiver validation contract {field} must be true when declared"
            ));
        }
    }
    Ok(())
}

fn peer_import_request_contract_summary(
    request: &PeerImportRequest,
) -> PeerImportRequestContractSummary {
    let metadata_routes_status = if request.metadata_routes.is_some() {
        "validated"
    } else {
        "not-declared"
    };
    let peer_status_route = request
        .metadata_routes
        .as_ref()
        .and_then(|routes| routes.peer_status.clone())
        .unwrap_or_else(|| PEER_STATUS_ROUTE.to_string());
    let peer_piece_set_route = request
        .metadata_routes
        .as_ref()
        .and_then(|routes| routes.peer_piece_set.clone())
        .unwrap_or_else(|| PEER_PIECE_SET_ROUTE.to_string());
    let peer_companion_state_route = request
        .metadata_routes
        .as_ref()
        .and_then(|routes| routes.peer_companion_state.clone())
        .unwrap_or_else(|| PEER_COMPANION_STATE_ROUTE.to_string());
    let receiver_validation_contract_status = if request.receiver_validation_contract.is_some() {
        "validated"
    } else {
        "not-declared"
    };
    let receiver_validation_contract = request.receiver_validation_contract.as_ref();
    PeerImportRequestContractSummary {
        metadata_routes_status: metadata_routes_status.to_string(),
        peer_status_route,
        peer_piece_set_route,
        peer_companion_state_route,
        receiver_validation_contract_status: receiver_validation_contract_status.to_string(),
        check_peer_status_before_piece_fetch: receiver_validation_contract
            .and_then(|contract| contract.check_peer_status_before_piece_fetch)
            .unwrap_or(false),
        preflight_fetches_piece_set_before_pieces: receiver_validation_contract
            .and_then(|contract| contract.preflight_fetches_piece_set_before_pieces)
            .unwrap_or(false),
        validate_advertised_companion_state_before_piece_fetch: receiver_validation_contract
            .and_then(|contract| contract.validate_advertised_companion_state_before_piece_fetch)
            .unwrap_or(false),
        import_receipt_required_before_browser_load: receiver_validation_contract
            .and_then(|contract| contract.import_receipt_required_before_browser_load)
            .unwrap_or(false),
    }
}

fn validate_peer_import_request_against_manifest(
    request: &PeerImportRequest,
    manifest: &Manifest,
) -> Result<(), String> {
    let manifest_model_id = manifest.require("model.id")?;
    if request.model_id != manifest_model_id {
        return Err(format!(
            "peer import model_id mismatch: request {}, manifest {}",
            request.model_id, manifest_model_id
        ));
    }
    let expected_byte_count = manifest
        .require("artifact.byte_count")?
        .parse::<u64>()
        .map_err(|_| "artifact.byte_count must be a decimal integer".to_string())?;
    if request.artifact_byte_count != expected_byte_count {
        return Err(format!(
            "peer import artifact_byte_count mismatch: request {}, manifest {}",
            request.artifact_byte_count, expected_byte_count
        ));
    }
    require_normalized_equal(
        "peer import artifact_sha256",
        &normalize_sha256(manifest.require("artifact.sha256")?)?,
        &normalize_sha256(&request.artifact_sha256)?,
        "request",
    )?;
    require_fixed_chunk_mode(manifest)?;
    Ok(())
}

#[derive(Debug, Clone)]
struct ImportReceiptPeerFields {
    peer_import: bool,
    peer_base_url: Option<String>,
    companion_state_status: Option<String>,
    piece_count: Option<usize>,
    merkle_root_sha256: Option<String>,
    peer_piece_bytes_fetched: bool,
    artifact_assembled_from_peer_pieces: bool,
}

fn validate_import_receipt_fields(
    receipt: &Manifest,
    manifest: &Manifest,
    artifact: &ArtifactVerification,
    artifact_path: &Path,
) -> Result<ImportReceiptPeerFields, String> {
    require_receipt_equal(receipt, "receipt.version", "0")?;
    require_receipt_equal(receipt, "receipt.kind", "minimodel.import_receipt")?;
    require_receipt_timestampish(receipt_required(receipt, "receipt.created_utc")?)?;
    require_receipt_sha_equal(receipt, "manifest.sha256", &artifact.manifest_sha256)?;
    require_receipt_equal(receipt, "manifest.model_id", &artifact.manifest_model_id)?;

    let file_name = artifact_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "artifact path must include a file name".to_string())?;
    require_receipt_equal(receipt, "artifact.local_path_hint", file_name)?;

    let receipt_byte_count = receipt_required(receipt, "artifact.byte_count")?
        .parse::<u64>()
        .map_err(|_| "import receipt artifact.byte_count must be a decimal integer".to_string())?;
    if receipt_byte_count != artifact.artifact_byte_count {
        return Err(format!(
            "import receipt artifact.byte_count mismatch: expected {}, got {receipt_byte_count}",
            artifact.artifact_byte_count
        ));
    }
    require_receipt_sha_equal(receipt, "artifact.sha256", &artifact.artifact_sha256)?;
    require_receipt_equal(receipt, "slm.validation_status", "passed")?;
    require_receipt_equal(receipt, "slm.model_shape", &artifact.slm.model_shape)?;
    require_receipt_equal(receipt, "slm.quantization", &artifact.slm.quantization)?;
    require_receipt_equal(receipt, "evidence.status", &artifact.evidence_status)?;
    require_receipt_equal(
        receipt,
        "import.next_gate",
        "user-selected-tinyrustlm-local-file-open",
    )?;

    validate_import_receipt_peer_fields(receipt, manifest)
}

fn validate_import_receipt_peer_fields(
    receipt: &Manifest,
    manifest: &Manifest,
) -> Result<ImportReceiptPeerFields, String> {
    let has_peer_fields = [
        "peer_import.transport",
        "peer_import.verification_status",
        "peer.base_url",
        "peer_piece_bytes_fetched",
        "artifact.assembled_from_peer_pieces",
    ]
    .iter()
    .any(|key| receipt.get(key).is_some());

    for key in [
        "project_server_serves_model_bytes",
        "project_server_runs_model_bytes",
        "project_server_proxies_huggingface_bytes",
    ] {
        require_receipt_optional_equal(receipt, key, "false")?;
    }

    if !has_peer_fields {
        return Ok(ImportReceiptPeerFields {
            peer_import: false,
            peer_base_url: None,
            companion_state_status: None,
            piece_count: None,
            merkle_root_sha256: None,
            peer_piece_bytes_fetched: false,
            artifact_assembled_from_peer_pieces: false,
        });
    }

    require_receipt_equal(
        receipt,
        "peer_import.transport",
        "known-direct-http-peer-v0",
    )?;
    require_receipt_equal(receipt, "peer_import.verification_status", "passed")?;
    require_safe_piece_file_name(receipt_required(receipt, "peer_import.request_path_hint")?)?;

    let peer_base_url = normalize_known_peer_base_url(receipt_required(receipt, "peer.base_url")?)?;
    require_safe_import_name(
        receipt_required(receipt, "peer.piece_dir_path_hint")?,
        "peer.piece_dir_path_hint",
    )?;
    require_receipt_equal(
        receipt,
        "peer.piece_set_path_hint",
        "minimodel.peer_piece_set.v0",
    )?;
    normalize_sha256(receipt_required(receipt, "peer.piece_set_sha256")?)?;

    let expected_companion_url = HttpUrl::parse(&peer_base_url)?
        .join_file(PEER_COMPANION_STATE_ROUTE)?
        .to_string();
    let companion_state_url = receipt_required(receipt, "peer.companion_state_url")?;
    if companion_state_url != expected_companion_url {
        return Err(format!(
            "import receipt peer.companion_state_url mismatch: expected {expected_companion_url}, got {companion_state_url}"
        ));
    }
    let companion_state_status = receipt_required(receipt, "peer.companion_state_status")?;
    require_value_member(
        companion_state_status,
        "peer.companion_state_status",
        &["validated", "not-advertised"],
    )?;

    require_fixed_chunk_mode(manifest)?;
    require_receipt_equal(receipt, "chunks.mode", "fixed-size-merkle-v0")?;
    let expected_piece_count = parse_usize(manifest.require("chunks.count")?, "chunks.count")?;
    let piece_count = parse_usize(
        receipt_required(receipt, "chunks.count")?,
        "import receipt chunks.count",
    )?;
    if piece_count != expected_piece_count {
        return Err(format!(
            "import receipt chunks.count mismatch: expected {expected_piece_count}, got {piece_count}"
        ));
    }
    let expected_merkle = normalize_sha256(manifest.require("chunks.merkle_root_sha256")?)?;
    let merkle_root_sha256 =
        normalize_sha256(receipt_required(receipt, "chunks.merkle_root_sha256")?)?;
    require_normalized_equal(
        "import receipt chunks.merkle_root_sha256",
        &expected_merkle,
        &merkle_root_sha256,
        "receipt",
    )?;

    require_receipt_bool(receipt, "peer_piece_set_metadata_fetched", true)?;
    require_receipt_bool(
        receipt,
        "peer_companion_state_metadata_fetched",
        companion_state_status == "validated",
    )?;
    let peer_piece_bytes_fetched = require_receipt_bool(receipt, "peer_piece_bytes_fetched", true)?;
    let artifact_assembled_from_peer_pieces =
        require_receipt_bool(receipt, "artifact.assembled_from_peer_pieces", true)?;

    Ok(ImportReceiptPeerFields {
        peer_import: true,
        peer_base_url: Some(peer_base_url),
        companion_state_status: Some(companion_state_status.to_string()),
        piece_count: Some(piece_count),
        merkle_root_sha256: Some(merkle_root_sha256),
        peer_piece_bytes_fetched,
        artifact_assembled_from_peer_pieces,
    })
}

fn receipt_required<'a>(receipt: &'a Manifest, key: &str) -> Result<&'a str, String> {
    receipt
        .get(key)
        .ok_or_else(|| format!("missing required import receipt field {key}"))
}

fn require_receipt_equal(receipt: &Manifest, key: &str, expected: &str) -> Result<(), String> {
    let actual = receipt_required(receipt, key)?;
    if actual == expected {
        return Ok(());
    }
    Err(format!(
        "import receipt {key} mismatch: expected {expected}, got {actual}"
    ))
}

fn require_receipt_optional_equal(
    receipt: &Manifest,
    key: &str,
    expected: &str,
) -> Result<(), String> {
    let Some(actual) = receipt.get(key) else {
        return Ok(());
    };
    if actual == expected {
        return Ok(());
    }
    Err(format!(
        "import receipt {key} mismatch: expected {expected}, got {actual}"
    ))
}

fn require_receipt_sha_equal(receipt: &Manifest, key: &str, expected: &str) -> Result<(), String> {
    let actual = normalize_sha256(receipt_required(receipt, key)?)?;
    require_normalized_equal(
        &format!("import receipt {key}"),
        &normalize_sha256(expected)?,
        &actual,
        "receipt",
    )
}

fn require_receipt_bool(receipt: &Manifest, key: &str, expected: bool) -> Result<bool, String> {
    let actual = receipt_required(receipt, key)?;
    let parsed = match actual {
        "true" => true,
        "false" => false,
        _ => return Err(format!("import receipt {key} must be true or false")),
    };
    if parsed == expected {
        return Ok(parsed);
    }
    Err(format!(
        "import receipt {key} mismatch: expected {expected}, got {actual}"
    ))
}

fn require_receipt_path_hint(
    receipt: &Manifest,
    key: &str,
    expected_path: &Path,
    label: &str,
) -> Result<(), String> {
    let actual_hint = path_file_name_hint(receipt_required(receipt, key)?, label)?;
    let expected_hint = expected_path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{label} path must include a file or directory name"))?;
    if actual_hint == expected_hint {
        return Ok(());
    }
    Err(format!(
        "import receipt {key} mismatch: expected basename {expected_hint}, got {actual_hint}"
    ))
}

fn require_receipt_timestampish(value: &str) -> Result<(), String> {
    parse_utc_timestamp_seconds(value, "import receipt receipt.created_utc").map(|_| ())
}

fn preflight_required<'a>(preflight: &'a Manifest, key: &str) -> Result<&'a str, String> {
    preflight
        .get(key)
        .ok_or_else(|| format!("missing required peer import preflight field {key}"))
}

fn require_preflight_equal(preflight: &Manifest, key: &str, expected: &str) -> Result<(), String> {
    let actual = preflight_required(preflight, key)?;
    if actual == expected {
        return Ok(());
    }
    Err(format!(
        "peer import preflight {key} mismatch: expected {expected}, got {actual}"
    ))
}

fn require_preflight_sha_equal(
    preflight: &Manifest,
    key: &str,
    expected: &str,
) -> Result<(), String> {
    let actual = normalize_sha256(preflight_required(preflight, key)?)?;
    require_normalized_equal(
        &format!("peer import preflight {key}"),
        &normalize_sha256(expected)?,
        &actual,
        "preflight",
    )
}

fn require_preflight_bool(preflight: &Manifest, key: &str, expected: bool) -> Result<bool, String> {
    let actual = preflight_required(preflight, key)?;
    let parsed = match actual {
        "true" => true,
        "false" => false,
        _ => return Err(format!("peer import preflight {key} must be true or false")),
    };
    if parsed == expected {
        return Ok(parsed);
    }
    Err(format!(
        "peer import preflight {key} mismatch: expected {expected}, got {actual}"
    ))
}

fn require_preflight_timestampish(value: &str) -> Result<(), String> {
    parse_utc_timestamp_seconds(value, "peer import preflight preflight.created_utc").map(|_| ())
}

fn require_preflight_path_hint(
    preflight: &Manifest,
    key: &str,
    expected_path: &Path,
    label: &str,
) -> Result<(), String> {
    let actual_hint = path_file_name_hint(preflight_required(preflight, key)?, label)?;
    let expected_hint = expected_path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{label} path must include a file or directory name"))?;
    if actual_hint == expected_hint {
        return Ok(());
    }
    Err(format!(
        "peer import preflight {key} mismatch: expected basename {expected_hint}, got {actual_hint}"
    ))
}

pub fn import_receipt_text(
    report: &ArtifactVerification,
    artifact_path: &Path,
) -> Result<String, String> {
    let file_name = artifact_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "artifact path must include a file name".to_string())?;
    let mut text = String::new();
    push_line(&mut text, "receipt.version", "0");
    push_line(&mut text, "receipt.kind", "minimodel.import_receipt");
    push_line(&mut text, "receipt.created_utc", &utc_now()?);
    push_line(&mut text, "manifest.sha256", &report.manifest_sha256);
    push_line(&mut text, "manifest.model_id", &report.manifest_model_id);
    push_line(&mut text, "artifact.local_path_hint", file_name);
    push_line(
        &mut text,
        "artifact.byte_count",
        &report.artifact_byte_count.to_string(),
    );
    push_line(&mut text, "artifact.sha256", &report.artifact_sha256);
    push_line(&mut text, "slm.validation_status", "passed");
    push_line(&mut text, "slm.model_shape", &report.slm.model_shape);
    push_line(&mut text, "slm.quantization", &report.slm.quantization);
    push_line(&mut text, "evidence.status", &report.evidence_status);
    push_line(
        &mut text,
        "import.next_gate",
        "user-selected-tinyrustlm-local-file-open",
    );
    Ok(text)
}

fn peer_import_receipt_text(
    report: &ArtifactVerification,
    artifact_path: &Path,
    transfer: &PeerImportReceiptTransfer,
) -> Result<String, String> {
    let mut text = import_receipt_text(report, artifact_path)?;
    push_line(
        &mut text,
        "peer_import.request_path_hint",
        &path_file_name_hint(&transfer.request_path, "peer import request")?,
    );
    push_line(
        &mut text,
        "peer_import.transport",
        "known-direct-http-peer-v0",
    );
    push_line(&mut text, "peer_import.verification_status", "passed");
    push_line(&mut text, "peer.base_url", &transfer.peer_base_url);
    push_line(
        &mut text,
        "peer.piece_dir_path_hint",
        &path_file_name_hint(&transfer.piece_dir_path, "peer piece directory")?,
    );
    push_line(
        &mut text,
        "peer.piece_set_path_hint",
        &path_file_name_hint(&transfer.piece_set_path, "peer piece set")?,
    );
    push_line(
        &mut text,
        "peer.piece_set_sha256",
        &transfer.piece_set_sha256,
    );
    push_line(
        &mut text,
        "peer.companion_state_url",
        &transfer.companion_state_url,
    );
    push_line(
        &mut text,
        "peer.companion_state_status",
        &transfer.companion_state_status,
    );
    push_line(&mut text, "chunks.mode", "fixed-size-merkle-v0");
    push_line(&mut text, "chunks.count", &transfer.piece_count.to_string());
    push_line(
        &mut text,
        "chunks.merkle_root_sha256",
        &transfer.merkle_root_sha256,
    );
    push_line(&mut text, "peer_piece_set_metadata_fetched", "true");
    push_line(
        &mut text,
        "peer_companion_state_metadata_fetched",
        if transfer.companion_state_status == "validated" {
            "true"
        } else {
            "false"
        },
    );
    push_line(&mut text, "peer_piece_bytes_fetched", "true");
    push_line(&mut text, "artifact.assembled_from_peer_pieces", "true");
    push_line(&mut text, "project_server_serves_model_bytes", "false");
    push_line(&mut text, "project_server_runs_model_bytes", "false");
    push_line(
        &mut text,
        "project_server_proxies_huggingface_bytes",
        "false",
    );
    Ok(text)
}

fn peer_import_preflight_text(
    report: &PeerImportPreflight,
    request: &PeerImportRequest,
    manifest: &Manifest,
) -> Result<String, String> {
    let mut text = String::new();
    push_line(&mut text, "preflight.version", "0");
    push_line(
        &mut text,
        "preflight.kind",
        "minimodel.peer_import_preflight",
    );
    push_line(&mut text, "preflight.created_utc", &utc_now()?);
    push_line(&mut text, "request.path", &report.request_path);
    push_line(&mut text, "request.kind", &request.request_kind);
    push_line(&mut text, "manifest.sha256", &report.manifest_sha256);
    push_line(
        &mut text,
        "manifest.model_id",
        manifest.require("model.id")?,
    );
    push_line(&mut text, "peer.base_url", &report.peer_base_url);
    push_line(
        &mut text,
        "metadata_routes.status",
        &report.metadata_routes_status,
    );
    push_line(
        &mut text,
        "metadata_routes.peer_status",
        &report.peer_status_route,
    );
    push_line(
        &mut text,
        "metadata_routes.peer_piece_set",
        &report.peer_piece_set_route,
    );
    push_line(
        &mut text,
        "metadata_routes.peer_companion_state",
        &report.peer_companion_state_route,
    );
    push_line(
        &mut text,
        "receiver_validation_contract.status",
        &report.receiver_validation_contract_status,
    );
    push_line(
        &mut text,
        "receiver_validation.check_peer_status_before_piece_fetch",
        if report.check_peer_status_before_piece_fetch {
            "true"
        } else {
            "false"
        },
    );
    push_line(
        &mut text,
        "receiver_validation.preflight_fetches_piece_set_before_pieces",
        if report.preflight_fetches_piece_set_before_pieces {
            "true"
        } else {
            "false"
        },
    );
    push_line(
        &mut text,
        "receiver_validation.validate_advertised_companion_state_before_piece_fetch",
        if report.validate_advertised_companion_state_before_piece_fetch {
            "true"
        } else {
            "false"
        },
    );
    push_line(
        &mut text,
        "receiver_validation.import_receipt_required_before_browser_load",
        if report.import_receipt_required_before_browser_load {
            "true"
        } else {
            "false"
        },
    );
    push_line(&mut text, "peer.piece_set_url", &report.piece_set_url);
    push_line(
        &mut text,
        "peer.companion_state_url",
        &report.companion_state_url,
    );
    push_line(
        &mut text,
        "peer.companion_state_status",
        &report.companion_state_status,
    );
    push_line(&mut text, "piece_set.kind", "minimodel.peer_piece_set.v0");
    push_line(&mut text, "piece_set.validation_status", "passed");
    push_line(&mut text, "piece_set.sha256", &report.piece_set_sha256);
    push_line(&mut text, "artifact.kind", &request.artifact_kind);
    push_line(
        &mut text,
        "artifact.byte_count",
        &request.artifact_byte_count.to_string(),
    );
    push_line(&mut text, "artifact.sha256", &report.artifact_sha256);
    push_line(&mut text, "chunks.mode", "fixed-size-merkle-v0");
    push_line(&mut text, "chunks.count", &report.piece_count.to_string());
    push_line(
        &mut text,
        "chunks.merkle_root_sha256",
        &report.merkle_root_sha256,
    );
    push_line(&mut text, "peer_piece_set_metadata_fetched", "true");
    push_line(
        &mut text,
        "peer_companion_state_metadata_fetched",
        if report.companion_state_status == "validated" {
            "true"
        } else {
            "false"
        },
    );
    push_line(&mut text, "peer_piece_bytes_fetched", "false");
    push_line(&mut text, "artifact_bytes_fetched", "false");
    push_line(&mut text, "project_server_serves_model_bytes", "false");
    push_line(&mut text, "project_server_runs_model_bytes", "false");
    push_line(
        &mut text,
        "project_server_proxies_huggingface_bytes",
        "false",
    );
    push_line(
        &mut text,
        "required_next_gate",
        "run-import-peer-request-fetch-pieces-assemble-rust-reverify",
    );
    Ok(text)
}

fn validate_peer_offer_against_manifest(
    manifest: &Manifest,
    peer_offer: &Manifest,
    peer_offer_sha256: &str,
) -> Result<(), String> {
    require_peer_offer_equal(peer_offer, "minimodel_peer_offer_version", "0")?;
    require_peer_offer_equal(peer_offer, "offer_kind", "minimodel.peer_offer.v0")?;
    require_peer_offer_equal(peer_offer, "artifact_kind", "slm")?;
    require_safe_piece_file_name(peer_offer.require("artifact_file_name")?)?;
    require_peer_offer_equal(
        peer_offer,
        "artifact_byte_count",
        manifest.require("artifact.byte_count")?,
    )?;
    require_peer_offer_equal(
        peer_offer,
        "artifact_sha256",
        &normalize_sha256(manifest.require("artifact.sha256")?)?,
    )?;
    require_peer_offer_equal(
        peer_offer,
        "slm_version",
        manifest.require("slm.format_version")?,
    )?;
    if let Some(expected) = manifest.get("slm.checksum") {
        require_peer_offer_equal(peer_offer, "slm_checksum", expected)?;
    }
    require_peer_offer_equal(
        peer_offer,
        "slm_model_shape",
        manifest.require("slm.model_shape")?,
    )?;
    require_peer_offer_equal(
        peer_offer,
        "slm_quantization",
        manifest.require("slm.quantization")?,
    )?;
    require_peer_offer_equal(
        peer_offer,
        "slm_tokenizer_checksum",
        manifest.require("slm.tokenizer_checksum")?,
    )?;
    require_peer_offer_equal(
        peer_offer,
        "slm_tensor_layout_checksum",
        manifest.require("slm.tensor_layout_checksum")?,
    )?;
    require_peer_offer_equal(peer_offer, "chunk_algorithm", "sha256-merkle-v0")?;
    require_peer_offer_equal(peer_offer, "chunk_size", manifest.require("chunks.size")?)?;
    require_peer_offer_equal(peer_offer, "chunk_count", manifest.require("chunks.count")?)?;
    require_peer_offer_equal(
        peer_offer,
        "chunk_list_sha256",
        &normalize_sha256(manifest.require("chunks.list.sha256")?)?,
    )?;
    require_peer_offer_equal(
        peer_offer,
        "merkle_root_sha256",
        &normalize_sha256(manifest.require("chunks.merkle_root_sha256")?)?,
    )?;
    require_peer_offer_equal(peer_offer, "project_server_serves_model_bytes", "false")?;
    require_peer_offer_equal(peer_offer, "project_server_runs_model_bytes", "false")?;
    if let Some(value) = peer_offer.get("transfer_network_available") {
        require_value_member(value, "transfer_network_available", &["false"])?;
    }
    if let Some(expected) = manifest.get("evidence.admission.sha256") {
        require_normalized_equal(
            "evidence.admission.sha256",
            &normalize_sha256(expected)?,
            peer_offer_sha256,
            "peer offer proof",
        )?;
    }
    let chunk_count = parse_usize(manifest.require("chunks.count")?, "chunks.count")?;
    for index in 0..chunk_count {
        let manifest_prefix = format!("chunk.{index}");
        let peer_prefix = format!("chunk.{index}");
        require_peer_offer_equal(
            peer_offer,
            &format!("{peer_prefix}.offset"),
            manifest.require(&format!("{manifest_prefix}.offset"))?,
        )?;
        require_peer_offer_equal(
            peer_offer,
            &format!("{peer_prefix}.byte_count"),
            manifest.require(&format!("{manifest_prefix}.byte_count"))?,
        )?;
        require_peer_offer_equal(
            peer_offer,
            &format!("{peer_prefix}.sha256"),
            &normalize_sha256(manifest.require(&format!("{manifest_prefix}.sha256"))?)?,
        )?;
    }
    Ok(())
}

fn catalog_entry_json(
    manifest: &Manifest,
    peer_offer: &Manifest,
    manifest_route: &str,
    peer_offer_route: &str,
    peer_offer_sha256: &str,
    source_huggingface_model_id: Option<&str>,
    known_peer_base_url: Option<&str>,
) -> Result<String, String> {
    let mut fields = Vec::new();
    fields.push(json_number_field("catalog_version", "0"));
    fields.push(json_string_field("entry_kind", "minimodel.catalog_entry"));
    fields.push(json_string_field("model_id", manifest.require("model.id")?));
    fields.push(json_string_field(
        "display_name",
        manifest.require("model.id")?,
    ));
    fields.push(json_string_field(
        "publisher_id",
        manifest.require("publisher.id")?,
    ));
    fields.push(json_string_field(
        "publisher_key_id",
        manifest.require("publisher.key_id")?,
    ));
    fields.push(json_string_field("manifest_route", manifest_route));
    fields.push(json_string_field(
        "model_card_route",
        manifest.require("model_card.route")?,
    ));
    fields.push(json_string_field(
        "license_route",
        manifest.require("license.route")?,
    ));
    fields.push(json_string_field("artifact_kind", "slm"));
    fields.push(json_string_field(
        "artifact_file_name",
        peer_offer.require("artifact_file_name")?,
    ));
    fields.push(json_string_field(
        "local_file_name_hint",
        peer_offer.require("artifact_file_name")?,
    ));
    fields.push(json_number_field(
        "artifact_byte_count",
        manifest.require("artifact.byte_count")?,
    ));
    fields.push(json_string_field(
        "artifact_sha256",
        &normalize_sha256(manifest.require("artifact.sha256")?)?,
    ));
    fields.push(json_null_field("artifact_project_server_url"));
    fields.push(json_bool_field("project_server_serves_model_bytes", false));
    fields.push(json_bool_field("project_server_runs_model_bytes", false));
    fields.push(json_string_field(
        "peer_hint_status",
        "peer-offer-ready-known-peer-needed",
    ));
    fields.push(json_string_field(
        "peer_import_status",
        "request-script-ready",
    ));
    fields.push(json_string_field("peer_offer_route", peer_offer_route));
    fields.push(json_string_field("peer_offer_sha256", peer_offer_sha256));
    if let Some(peer_base_url) = known_peer_base_url {
        fields.push(json_number_field("peer_source_count", "1"));
        fields.push(json_raw_field(
            "peer_sources",
            &known_peer_sources_json(peer_base_url),
        ));
    } else {
        fields.push(json_number_field("peer_source_count", "0"));
    }
    fields.push(json_string_field(
        "peer_offer_kind",
        "minimodel.peer_offer.v0",
    ));
    fields.push(json_string_field(
        "peer_offer_chunk_algorithm",
        "sha256-merkle-v0",
    ));
    fields.push(json_number_field(
        "peer_offer_chunk_size",
        manifest.require("chunks.size")?,
    ));
    fields.push(json_number_field(
        "peer_offer_chunk_count",
        manifest.require("chunks.count")?,
    ));
    fields.push(json_string_field(
        "peer_offer_merkle_root_sha256",
        &normalize_sha256(manifest.require("chunks.merkle_root_sha256")?)?,
    ));
    fields.push(json_string_field(
        "execution_policy",
        "user-local-file-only",
    ));
    fields.push(json_string_field(
        "source_kind",
        manifest.require("source.kind")?,
    ));
    fields.push(json_string_field(
        "source_identity",
        manifest.require("source.id")?,
    ));
    fields.push(json_string_field(
        "source_revision",
        manifest.require("source.revision")?,
    ));
    if let Some(model_id) = source_huggingface_model_id {
        fields.push(json_string_field("source_huggingface_model_id", model_id));
    }
    fields.push(json_string_field(
        "slm_model_shape",
        manifest.require("slm.model_shape")?,
    ));
    fields.push(json_string_field(
        "slm_quantization",
        manifest.require("slm.quantization")?,
    ));
    if let Some(value) = manifest.get("slm.checksum") {
        fields.push(json_string_field("slm_checksum", value));
    }
    fields.push(json_string_field(
        "slm_tokenizer_checksum",
        manifest.require("slm.tokenizer_checksum")?,
    ));
    fields.push(json_string_field(
        "slm_tensor_layout_checksum",
        manifest.require("slm.tensor_layout_checksum")?,
    ));
    if let Some(value) = manifest.get("source.config.sha256") {
        fields.push(json_string_field("slm_config_checksum", value));
    }
    fields.push(json_string_field(
        "quality_claim",
        "not-endorsed-by-project",
    ));
    fields.push(json_string_field("trained_quality_claim", "not-claimed"));
    fields.push(json_string_field(
        "evidence_admission_status",
        manifest.require("evidence.admission.status")?,
    ));
    fields.push(json_string_field(
        "required_next_gate",
        "known-consenting-peer-fetch-assemble-rust-reverify-before-browser-import",
    ));
    let mut text = String::new();
    text.push_str("{\n");
    for (index, field) in fields.iter().enumerate() {
        text.push_str("  ");
        text.push_str(field);
        if index + 1 != fields.len() {
            text.push(',');
        }
        text.push('\n');
    }
    text.push_str("}\n");
    Ok(text)
}

#[derive(Debug)]
struct CatalogEntrySummary {
    model_id: String,
    peer_source_count: usize,
}

#[derive(Debug)]
struct CatalogSnapshotSummary {
    entry_count: usize,
    p2p_ready_count: usize,
    peer_source_count: usize,
    announcement_route_count: usize,
}

fn catalog_snapshot_value(entries: Vec<Value>) -> Result<Value, String> {
    catalog_snapshot_value_with_announcement_routes(entries, Vec::new())
}

fn catalog_snapshot_value_with_announcement_routes(
    entries: Vec<Value>,
    announcement_routes: Vec<Value>,
) -> Result<Value, String> {
    let mut object = Map::new();
    object.insert("catalog_version".to_string(), Value::Number(0.into()));
    object.insert(
        "catalog_kind".to_string(),
        Value::String("minimodel.model_catalog".to_string()),
    );
    object.insert(
        "catalog_id".to_string(),
        Value::String("minimodel-local-generated-v0".to_string()),
    );
    object.insert("generated_at".to_string(), Value::String(utc_now()?));
    object.insert(
        "publisher_id".to_string(),
        Value::String("local-user".to_string()),
    );
    object.insert(
        "publisher_key_id".to_string(),
        Value::String("local-user-unsigned".to_string()),
    );
    object.insert(
        "project_server_serves_model_bytes".to_string(),
        Value::Bool(false),
    );
    object.insert(
        "project_server_runs_model_bytes".to_string(),
        Value::Bool(false),
    );
    object.insert("artifact_project_server_url".to_string(), Value::Null);
    object.insert(
        "execution_policy".to_string(),
        Value::String("user-local-file-or-consenting-peer-only".to_string()),
    );
    if !announcement_routes.is_empty() {
        object.insert(
            "peer_source_announcement_routes".to_string(),
            Value::Array(announcement_routes),
        );
    }
    object.insert("entries".to_string(), Value::Array(entries));
    Ok(Value::Object(object))
}

fn catalog_announcement_route_value(route: &str, feed_sha256: &str) -> Result<Value, String> {
    let route = validate_catalog_announcement_route(route)?;
    let feed_sha256 = normalize_sha256(feed_sha256)?;
    let mut object = Map::new();
    object.insert("route".to_string(), Value::String(route));
    object.insert("sha256".to_string(), Value::String(feed_sha256));
    object.insert(
        "route_kind".to_string(),
        Value::String("minimodel.peer_source_announcement_catalog".to_string()),
    );
    Ok(Value::Object(object))
}

fn validate_catalog_announcement_route(value: &str) -> Result<String, String> {
    let route = value.trim();
    if route.is_empty() || route.len() > 240 {
        return Err("peer-source announcement route must be 1..=240 bytes".to_string());
    }
    if route.contains("://") || route.contains('\\') || route.contains('?') || route.contains('#') {
        return Err(
            "peer-source announcement route must be same-origin path metadata, not a URL or query"
                .to_string(),
        );
    }
    if !route
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        return Err("peer-source announcement route must be route-safe ASCII".to_string());
    }
    for segment in route.split('/') {
        if segment == "." || segment == ".." {
            return Err(
                "peer-source announcement route must not contain dot path segments".to_string(),
            );
        }
    }
    Ok(route.to_string())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShareMetadataTransferMode {
    MetadataOnly,
    ShareKitReady,
}

#[derive(Debug, Clone)]
struct ShareMetadataPeerOfferSummary {
    artifact_file_name: String,
    artifact_byte_count: usize,
    artifact_sha256: String,
    chunk_count: usize,
    merkle_root_sha256: String,
    network_transfer_available: bool,
}

fn validate_share_metadata_bundle_value(
    value: &Value,
    label: &str,
    bundle_path: String,
) -> Result<ShareMetadataReport, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{label} share metadata bundle must be a JSON object"))?;
    require_json_string_field(
        value,
        "bundle_kind",
        label,
        "minimodel.local_slm_share_metadata.v0",
    )?;
    json_required_string(value, "generated_at", label)?;

    let boundary_label = format!("{label} boundary");
    let boundary = json_required_object(value, "boundary", label)?;
    let (mode, peer_transfer_status, peer_transfer_implementation) =
        validate_share_metadata_boundary_value(boundary, &boundary_label)?;

    let catalog_label = format!("{label} catalog_entry");
    let catalog_entry = json_required_object(value, "catalog_entry", label)?;
    let catalog_summary = validate_catalog_entry_value(catalog_entry, &catalog_label)?;
    let catalog_artifact_file_name =
        json_required_string(catalog_entry, "artifact_file_name", &catalog_label)?;
    require_safe_piece_file_name(&catalog_artifact_file_name)?;
    if !catalog_artifact_file_name.ends_with(".slm") {
        return Err(format!(
            "{catalog_label} artifact_file_name must end with .slm"
        ));
    }
    let catalog_artifact_byte_count =
        json_required_usize(catalog_entry, "artifact_byte_count", &catalog_label)?;
    let catalog_artifact_sha256 = normalize_sha256(&json_required_string(
        catalog_entry,
        "artifact_sha256",
        &catalog_label,
    )?)?;
    if let Some(source_model_id) =
        json_optional_string(catalog_entry, "source_huggingface_model_id")
    {
        validate_huggingface_model_id(&source_model_id)?;
    }
    if catalog_entry.get("peer_sources").is_some() {
        return Err(format!(
            "{catalog_label} must not include peer_sources; local share metadata is not a catalog availability claim"
        ));
    }

    let peer_offer_label = format!("{label} peer_offer");
    let peer_offer = json_required_object(value, "peer_offer", label)?;
    let peer_offer_summary =
        validate_share_metadata_peer_offer_value(peer_offer, &peer_offer_label, mode)?;
    if peer_offer_summary.artifact_file_name != catalog_artifact_file_name {
        return Err(format!(
            "{peer_offer_label} artifact_file_name {} did not match catalog artifact_file_name {}",
            peer_offer_summary.artifact_file_name, catalog_artifact_file_name
        ));
    }
    if peer_offer_summary.artifact_byte_count != catalog_artifact_byte_count {
        return Err(format!(
            "{peer_offer_label} artifact_byte_count {} did not match catalog artifact_byte_count {}",
            peer_offer_summary.artifact_byte_count, catalog_artifact_byte_count
        ));
    }
    require_normalized_equal(
        "artifact_sha256",
        &catalog_artifact_sha256,
        &peer_offer_summary.artifact_sha256,
        &peer_offer_label,
    )?;

    validate_share_metadata_conversion_request(value, label)?;
    validate_share_metadata_share_kit(value, label, mode)?;

    Ok(ShareMetadataReport {
        bundle_path,
        model_id: catalog_summary.model_id,
        artifact_file_name: catalog_artifact_file_name,
        artifact_byte_count: catalog_artifact_byte_count,
        artifact_sha256: catalog_artifact_sha256,
        chunk_count: peer_offer_summary.chunk_count,
        merkle_root_sha256: peer_offer_summary.merkle_root_sha256,
        peer_transfer_status,
        peer_transfer_implementation,
        share_kit_available: mode == ShareMetadataTransferMode::ShareKitReady,
        network_transfer_available: peer_offer_summary.network_transfer_available,
    })
}

fn validate_share_metadata_boundary_value(
    boundary: &Value,
    label: &str,
) -> Result<(ShareMetadataTransferMode, String, String), String> {
    require_json_bool_false(boundary, "project_server_runs_model_bytes", label)?;
    require_json_bool_false(boundary, "project_server_serves_model_bytes", label)?;
    require_json_bool_true(boundary, "user_consent_required_before_transfer", label)?;
    json_required_string(boundary, "tiny_rust_lm_execution_rule", label)?;

    let implemented = json_required_bool(boundary, "peer_transfer_implemented", label)?;
    let status = json_required_string(boundary, "peer_transfer_status", label)?;
    let implementation = json_required_string(boundary, "peer_transfer_implementation", label)?;
    let mode = match (implemented, status.as_str(), implementation.as_str()) {
        (false, "metadata-only", "not-included-in-metadata-export") => {
            ShareMetadataTransferMode::MetadataOnly
        }
        (
            true,
            "local-share-kit-script-ready-known-peer-needed",
            "generated-local-minimodel-share-kit-v0",
        ) => ShareMetadataTransferMode::ShareKitReady,
        _ => {
            return Err(format!(
                "{label} has unsupported peer transfer profile: implemented={implemented}, status={status}, implementation={implementation}"
            ));
        }
    };
    Ok((mode, status, implementation))
}

fn validate_share_metadata_peer_offer_value(
    peer_offer: &Value,
    label: &str,
    mode: ShareMetadataTransferMode,
) -> Result<ShareMetadataPeerOfferSummary, String> {
    require_json_string_field(peer_offer, "offer_kind", label, "minimodel.peer_offer.v0")?;
    require_json_u64_field(peer_offer, "offer_version", label, 0)?;
    require_json_string_field(peer_offer, "artifact_kind", label, "slm")?;
    match mode {
        ShareMetadataTransferMode::MetadataOnly => require_json_string_field(
            peer_offer,
            "offer_status",
            label,
            "metadata-ready-transfer-not-implemented",
        )?,
        ShareMetadataTransferMode::ShareKitReady => require_json_string_field(
            peer_offer,
            "offer_status",
            label,
            "local-share-kit-ready-known-peer-needed",
        )?,
    }

    let artifact_file_name = json_required_string(peer_offer, "artifact_file_name", label)?;
    require_safe_piece_file_name(&artifact_file_name)?;
    if !artifact_file_name.ends_with(".slm") {
        return Err(format!("{label} artifact_file_name must end with .slm"));
    }
    let artifact_byte_count = json_required_usize(peer_offer, "artifact_byte_count", label)?;
    if artifact_byte_count == 0 {
        return Err(format!("{label} artifact_byte_count must be positive"));
    }
    let artifact_sha256 =
        normalize_sha256(&json_required_string(peer_offer, "artifact_sha256", label)?)?;
    json_required_string(peer_offer, "slm_checksum", label)?;

    let chunks_label = format!("{label} chunks");
    let chunks_value = json_required_object(peer_offer, "chunks", label)?;
    require_json_string_field(chunks_value, "algorithm", &chunks_label, "sha256-merkle-v0")?;
    let chunk_size = json_required_usize(chunks_value, "chunk_size", &chunks_label)?;
    if chunk_size == 0 {
        return Err(format!("{chunks_label} chunk_size must be positive"));
    }
    let chunk_count = json_required_usize(chunks_value, "chunk_count", &chunks_label)?;
    let entries = json_required_array(chunks_value, "entries", &chunks_label)?;
    if chunk_count != entries.len() {
        return Err(format!(
            "{chunks_label} chunk_count {chunk_count} did not match entries length {}",
            entries.len()
        ));
    }
    if chunk_count == 0 {
        return Err(format!("{chunks_label} chunk_count must be positive"));
    }

    let mut chunks = Vec::with_capacity(entries.len());
    let mut expected_offset = 0usize;
    for (index, entry) in entries.iter().enumerate() {
        let entry_label = format!("{chunks_label} entries[{index}]");
        let entry_index = json_required_usize(entry, "index", &entry_label)?;
        if entry_index != index {
            return Err(format!(
                "{entry_label} index {entry_index} did not match position {index}"
            ));
        }
        let offset = json_required_usize(entry, "offset", &entry_label)?;
        if offset != expected_offset {
            return Err(format!(
                "{entry_label} offset {offset} did not match expected offset {expected_offset}"
            ));
        }
        let byte_count = json_required_usize(entry, "byte_count", &entry_label)?;
        if byte_count == 0 {
            return Err(format!("{entry_label} byte_count must be positive"));
        }
        if byte_count > chunk_size {
            return Err(format!(
                "{entry_label} byte_count {byte_count} exceeds chunk_size {chunk_size}"
            ));
        }
        if index + 1 < entries.len() && byte_count != chunk_size {
            return Err(format!(
                "{entry_label} byte_count must equal chunk_size except for the final chunk"
            ));
        }
        let sha256 = normalize_sha256(&json_required_string(entry, "sha256", &entry_label)?)?;
        chunks.push(ChunkProof {
            offset,
            byte_count,
            sha256,
        });
        expected_offset = expected_offset
            .checked_add(byte_count)
            .ok_or_else(|| format!("{entry_label} offset overflow"))?;
    }
    if expected_offset != artifact_byte_count {
        return Err(format!(
            "{chunks_label} byte total {expected_offset} did not match artifact_byte_count {artifact_byte_count}"
        ));
    }
    if chunks.len() == 1 && chunks[0].sha256 != artifact_sha256 {
        return Err(format!(
            "{chunks_label} single chunk SHA-256 {} did not match artifact_sha256 {}",
            chunks[0].sha256, artifact_sha256
        ));
    }

    let chunk_list_sha256 = normalize_sha256(&json_required_string(
        chunks_value,
        "chunk_list_sha256",
        &chunks_label,
    )?)?;
    let actual_chunk_list_sha256 = sha256_prefixed(chunk_list_text(&chunks).as_bytes());
    if chunk_list_sha256 != actual_chunk_list_sha256 {
        return Err(format!(
            "{chunks_label} chunk_list_sha256 {chunk_list_sha256} did not match recomputed {actual_chunk_list_sha256}"
        ));
    }
    let declared_merkle_root_sha256 = normalize_sha256(&json_required_string(
        chunks_value,
        "merkle_root_sha256",
        &chunks_label,
    )?)?;
    let actual_merkle_root_sha256 = merkle_root_sha256(&chunks)?;
    if declared_merkle_root_sha256 != actual_merkle_root_sha256 {
        return Err(format!(
            "{chunks_label} merkle_root_sha256 {declared_merkle_root_sha256} did not match recomputed {actual_merkle_root_sha256}"
        ));
    }

    let transfer_label = format!("{label} transfer");
    let transfer = json_required_object(peer_offer, "transfer", label)?;
    let protocols = json_required_string_array(transfer, "protocols", &transfer_label)?;
    match mode {
        ShareMetadataTransferMode::MetadataOnly => {
            if !protocols.is_empty() {
                return Err(format!(
                    "{transfer_label} protocols must be empty for metadata-only exports"
                ));
            }
            require_json_bool_false(transfer, "local_share_kit_available", &transfer_label)?;
            require_json_bool_false(transfer, "serve_helper_available", &transfer_label)?;
            require_json_string_field(
                transfer,
                "required_next_gate",
                &transfer_label,
                "implement-consent-based-peer-transfer-and-rust-reverify-before-import",
            )?;
        }
        ShareMetadataTransferMode::ShareKitReady => {
            let expected_protocols = [
                "minimodel-prepare-peer-share-v0",
                "known-direct-http-peer-v0",
                "minimodel-peer-companion-v0",
            ];
            if protocols != expected_protocols {
                return Err(format!(
                    "{transfer_label} protocols must be {:?}, got {:?}",
                    expected_protocols, protocols
                ));
            }
            require_json_bool_true(transfer, "local_share_kit_available", &transfer_label)?;
            require_json_bool_true(transfer, "serve_helper_available", &transfer_label)?;
            require_json_string_field(
                transfer,
                "required_next_gate",
                &transfer_label,
                "run-generated-share-kit-set-known-peer-url-and-serve-from-user-machine",
            )?;
        }
    }
    require_json_bool_false(transfer, "network_transfer_available", &transfer_label)?;
    require_json_bool_false(
        transfer,
        "project_server_serves_model_bytes",
        &transfer_label,
    )?;
    require_json_bool_false(transfer, "project_server_runs_model_bytes", &transfer_label)?;
    let network_transfer_available =
        json_required_bool(transfer, "network_transfer_available", &transfer_label)?;

    Ok(ShareMetadataPeerOfferSummary {
        artifact_file_name,
        artifact_byte_count,
        artifact_sha256,
        chunk_count,
        merkle_root_sha256: declared_merkle_root_sha256,
        network_transfer_available,
    })
}

fn validate_share_metadata_conversion_request(value: &Value, label: &str) -> Result<(), String> {
    let Some(conversion_request) = value.get("conversion_request") else {
        return Ok(());
    };
    if conversion_request.is_null() {
        return Ok(());
    }
    let conversion_label = format!("{label} conversion_request");
    conversion_request
        .as_object()
        .ok_or_else(|| format!("{conversion_label} must be an object or null"))?;
    require_json_string_field(
        conversion_request,
        "request_kind",
        &conversion_label,
        "minimodel.huggingface_conversion_request.v0",
    )?;
    if let Some(source) = conversion_request.get("source") {
        let source_label = format!("{conversion_label} source");
        source
            .as_object()
            .ok_or_else(|| format!("{source_label} must be an object"))?;
        if let Some(model_id) = json_optional_string(source, "model_id") {
            validate_huggingface_model_id(&model_id)?;
        }
        if let Some(kind) = json_optional_string(source, "kind") {
            if kind != "huggingface_model" {
                return Err(format!(
                    "{source_label} kind must be huggingface_model, got {kind}"
                ));
            }
        }
    }
    Ok(())
}

fn validate_share_metadata_share_kit(
    value: &Value,
    label: &str,
    mode: ShareMetadataTransferMode,
) -> Result<(), String> {
    match (mode, value.get("share_kit")) {
        (ShareMetadataTransferMode::MetadataOnly, None) => Ok(()),
        (ShareMetadataTransferMode::MetadataOnly, Some(_)) => Err(format!(
            "{label} metadata-only export must not include share_kit"
        )),
        (ShareMetadataTransferMode::ShareKitReady, None) => {
            Err(format!("{label} share-kit-ready export missing share_kit"))
        }
        (ShareMetadataTransferMode::ShareKitReady, Some(share_kit)) => {
            let share_kit_label = format!("{label} share_kit");
            share_kit
                .as_object()
                .ok_or_else(|| format!("{share_kit_label} must be an object"))?;
            require_json_string_field(
                share_kit,
                "kit_kind",
                &share_kit_label,
                "tinyrustlm.local_p2p_share_kit.v0",
            )?;
            require_json_string_field(
                share_kit,
                "kit_status",
                &share_kit_label,
                "script-ready-known-peer-needed",
            )?;
            for key in [
                "prepares_peer_pieces",
                "writes_catalog_snapshot",
                "writes_peer_source_metadata_when_known_peer_set",
                "writes_peer_companion_state_when_known_peer_set",
                "writes_peer_contact_card_when_known_peer_set",
                "serve_helper_available",
                "user_consent_required_before_transfer",
            ] {
                require_json_bool_true(share_kit, key, &share_kit_label)?;
            }
            require_json_bool_false(
                share_kit,
                "project_server_serves_model_bytes",
                &share_kit_label,
            )?;
            require_json_bool_false(
                share_kit,
                "project_server_runs_model_bytes",
                &share_kit_label,
            )?;
            json_required_string(share_kit, "execution_rule", &share_kit_label)?;
            Ok(())
        }
    }
}

fn validate_catalog_entry_value(value: &Value, label: &str) -> Result<CatalogEntrySummary, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{label} catalog entry must be a JSON object"))?;
    require_json_u64_field(value, "catalog_version", label, 0)?;
    require_json_string_field(value, "entry_kind", label, "minimodel.catalog_entry")?;
    require_json_string_field(value, "artifact_kind", label, "slm")?;
    require_json_bool_false(value, "project_server_serves_model_bytes", label)?;
    require_json_bool_false(value, "project_server_runs_model_bytes", label)?;
    reject_catalog_artifact_url_fields(object, label)?;

    let model_id = json_required_string(value, "model_id", label)?;
    validate_minimodel_model_id(&model_id)?;
    let artifact_byte_count = json_required_usize(value, "artifact_byte_count", label)?;
    if artifact_byte_count == 0 {
        return Err(format!("{label} artifact_byte_count must be positive"));
    }
    normalize_sha256(&json_required_string(value, "artifact_sha256", label)?)?;

    let declared_peer_source_count =
        json_optional_usize(value, "peer_source_count", label)?.unwrap_or(0);
    let actual_peer_source_count = validate_catalog_peer_sources(value, label)?;
    if declared_peer_source_count != actual_peer_source_count {
        return Err(format!(
            "{label} peer_source_count {declared_peer_source_count} did not match peer_sources length {actual_peer_source_count}"
        ));
    }

    let peer_import_status = json_optional_string(value, "peer_import_status");
    let peer_hint_status = json_optional_string(value, "peer_hint_status");
    let has_peer_offer_fields = [
        "peer_offer_route",
        "peer_offer_sha256",
        "peer_offer_chunk_count",
        "peer_offer_merkle_root_sha256",
    ]
    .iter()
    .any(|key| value.get(*key).is_some());
    if peer_import_status.as_deref() == Some("request-script-ready")
        || peer_hint_status.as_deref() == Some("peer-offer-ready-known-peer-needed")
        || declared_peer_source_count > 0
        || has_peer_offer_fields
    {
        validate_catalog_peer_offer_proof_fields(value, label)?;
    }

    Ok(CatalogEntrySummary {
        model_id,
        peer_source_count: declared_peer_source_count,
    })
}

fn validate_catalog_snapshot_value(
    value: &Value,
    label: &str,
) -> Result<CatalogSnapshotSummary, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{label} catalog snapshot must be a JSON object"))?;
    require_json_u64_field(value, "catalog_version", label, 0)?;
    require_json_string_field(value, "catalog_kind", label, "minimodel.model_catalog")?;
    json_required_string(value, "catalog_id", label)?;
    json_required_string(value, "generated_at", label)?;
    json_required_string(value, "publisher_id", label)?;
    json_required_string(value, "publisher_key_id", label)?;
    require_json_bool_false(value, "project_server_serves_model_bytes", label)?;
    require_json_bool_false(value, "project_server_runs_model_bytes", label)?;
    reject_catalog_artifact_url_fields(object, label)?;
    json_required_string(value, "execution_policy", label)?;

    let announcement_route_count = validate_catalog_announcement_route_fields(value, label)?;
    let entries = json_required_array(value, "entries", label)?;
    if entries.is_empty() {
        return Err(format!("{label} catalog entries must not be empty"));
    }
    if entries.len() > 4096 {
        return Err(format!("{label} catalog entries exceeded 4096 rows"));
    }

    let mut model_ids = BTreeSet::new();
    let mut p2p_ready_count = 0usize;
    let mut peer_source_count = 0usize;
    for (index, entry) in entries.iter().enumerate() {
        let entry_label = format!("{label} entries[{index}]");
        let summary = validate_catalog_entry_value(entry, &entry_label)?;
        if !model_ids.insert(summary.model_id.clone()) {
            return Err(format!(
                "{entry_label} duplicated catalog model_id {}",
                summary.model_id
            ));
        }
        if summary.peer_source_count > 0 {
            p2p_ready_count += 1;
            peer_source_count += summary.peer_source_count;
        }
    }

    Ok(CatalogSnapshotSummary {
        entry_count: entries.len(),
        p2p_ready_count,
        peer_source_count,
        announcement_route_count,
    })
}

fn validate_catalog_announcement_route_fields(value: &Value, label: &str) -> Result<usize, String> {
    let mut routes = BTreeSet::new();

    if value.get("peer_source_announcement_route").is_some() {
        let route = validate_catalog_announcement_route(&json_required_string(
            value,
            "peer_source_announcement_route",
            label,
        )?)?;
        let sha256 = json_optional_string(value, "peer_source_announcement_sha256")
            .or_else(|| json_optional_string(value, "peer_source_announcement_checksum_sha256"))
            .ok_or_else(|| {
                format!(
                    "{label} peer_source_announcement_route must include peer_source_announcement_sha256"
                )
            })?;
        normalize_sha256(&sha256)?;
        routes.insert(route);
    }

    for key in [
        "peer_source_announcement_routes",
        "peer_source_announcement_feeds",
    ] {
        let Some(raw_routes) = value.get(key) else {
            continue;
        };
        let raw_routes = raw_routes
            .as_array()
            .ok_or_else(|| format!("{label} {key} must be an array"))?;
        for (index, raw_route) in raw_routes.iter().enumerate() {
            let route_label = format!("{label} {key}[{index}]");
            let route = validate_catalog_announcement_route_reference(raw_route, &route_label)?;
            if !routes.insert(route.clone()) {
                return Err(format!("{route_label} duplicated route {route}"));
            }
        }
    }

    if routes.len() > 64 {
        return Err(format!(
            "{label} listed more than 64 peer-source announcement routes"
        ));
    }
    Ok(routes.len())
}

fn validate_catalog_announcement_route_reference(
    value: &Value,
    label: &str,
) -> Result<String, String> {
    if let Some(route) = value.as_str() {
        validate_catalog_announcement_route(route)?;
        return Err(format!("{label} must include a SHA-256 checksum"));
    }

    let object = value
        .as_object()
        .ok_or_else(|| format!("{label} must be an object with route and sha256"))?;
    let route = ["route", "announcement_route", "url", "href"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(validate_catalog_announcement_route)
        .transpose()?
        .ok_or_else(|| format!("{label} missing string route"))?;
    let sha256 = [
        "sha256",
        "checksum_sha256",
        "announcement_sha256",
        "peer_source_announcement_sha256",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_str))
    .ok_or_else(|| format!("{label} must include a SHA-256 checksum"))?;
    normalize_sha256(sha256)?;
    if let Some(route_kind) = object.get("route_kind").and_then(Value::as_str) {
        if route_kind != "minimodel.peer_source_announcement_catalog" {
            return Err(format!(
                "{label} route_kind must be minimodel.peer_source_announcement_catalog"
            ));
        }
    }
    Ok(route)
}

fn validate_catalog_peer_offer_proof_fields(value: &Value, label: &str) -> Result<(), String> {
    require_json_string_field(value, "peer_offer_kind", label, "minimodel.peer_offer.v0")?;
    require_json_string_field(
        value,
        "peer_offer_chunk_algorithm",
        label,
        "sha256-merkle-v0",
    )?;
    let route = json_required_string(value, "peer_offer_route", label)?;
    if route.contains('\n') || route.contains('\r') {
        return Err(format!(
            "{label} peer_offer_route must not contain newlines"
        ));
    }
    normalize_sha256(&json_required_string(value, "peer_offer_sha256", label)?)?;
    let chunk_size = json_required_usize(value, "peer_offer_chunk_size", label)?;
    if chunk_size == 0 {
        return Err(format!("{label} peer_offer_chunk_size must be positive"));
    }
    let chunk_count = json_required_usize(value, "peer_offer_chunk_count", label)?;
    if chunk_count == 0 {
        return Err(format!("{label} peer_offer_chunk_count must be positive"));
    }
    normalize_sha256(&json_required_string(
        value,
        "peer_offer_merkle_root_sha256",
        label,
    )?)?;
    Ok(())
}

fn reject_catalog_artifact_url_fields(
    object: &Map<String, Value>,
    label: &str,
) -> Result<(), String> {
    for key in [
        "artifact_project_server_url",
        "artifact_url",
        "model_url",
        "download_url",
        "resolve_url",
    ] {
        let Some(value) = object.get(key) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        if value.as_str().map(str::trim).unwrap_or("").is_empty() {
            continue;
        }
        return Err(format!(
            "{label} must not include {key}; catalog snapshots are metadata only"
        ));
    }
    Ok(())
}

fn validate_catalog_peer_sources(value: &Value, label: &str) -> Result<usize, String> {
    let Some(peer_sources) = value.get("peer_sources") else {
        return Ok(0);
    };
    let peer_sources = peer_sources
        .as_array()
        .ok_or_else(|| format!("{label} peer_sources must be an array"))?;
    for (index, peer_source) in peer_sources.iter().enumerate() {
        let peer_label = format!("{label} peer_sources[{index}]");
        validate_catalog_peer_source_value(peer_source, &peer_label)?;
    }
    Ok(peer_sources.len())
}

fn validate_catalog_peer_source_value(value: &Value, label: &str) -> Result<String, String> {
    require_json_string_field(value, "source_kind", label, "known-direct-http-peer-v0")?;
    require_json_string_field(value, "transport", label, "direct-http-piece-server-v0")?;
    require_json_string_field(
        value,
        "consent_status",
        label,
        "publisher-declared-consenting-peer",
    )?;
    require_json_bool_false(value, "project_server_serves_model_bytes", label)?;
    require_json_bool_false(value, "project_server_runs_model_bytes", label)?;
    normalize_known_peer_base_url(&json_required_string(value, "peer_base_url", label)?)
}

fn read_peer_source_announcement_path(
    path: &Path,
) -> Result<(String, PeerSourceAnnouncement), String> {
    let (label, _value, announcement) = read_peer_source_announcement_value_path(path)?;
    Ok((label, announcement))
}

fn read_peer_source_announcement_value_path(
    path: &Path,
) -> Result<(String, Value, PeerSourceAnnouncement), String> {
    let announcement_label = path.display().to_string();
    let text = fs::read_to_string(path).map_err(|error| {
        format!("failed to read peer source announcement {announcement_label}: {error}")
    })?;
    let announcement_value: Value = serde_json::from_str(&text).map_err(|error| {
        format!("failed to parse peer source announcement {announcement_label}: {error}")
    })?;
    let announcement =
        validate_peer_source_announcement_value(&announcement_value, &announcement_label)?;
    Ok((announcement_label, announcement_value, announcement))
}

fn validate_peer_source_announcement_feed_value(
    value: &Value,
    label: &str,
) -> Result<Vec<(String, PeerSourceAnnouncement)>, String> {
    require_json_u64_field(value, "announcement_catalog_version", label, 0)?;
    require_json_string_field(
        value,
        "announcement_catalog_kind",
        label,
        "minimodel.peer_source_announcement_catalog",
    )?;
    require_json_bool_false(value, "project_server_serves_model_bytes", label)?;
    require_json_bool_false(value, "project_server_runs_model_bytes", label)?;
    require_json_bool_false(value, "project_server_proxies_model_bytes", label)?;

    let raw_announcements = value
        .get("announcements")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{label} missing array announcements"))?;
    if raw_announcements.len() > 1024 {
        return Err(format!(
            "{label} peer-source announcement feed exceeded 1024 announcements"
        ));
    }

    let mut seen = BTreeSet::new();
    let mut announcements = Vec::with_capacity(raw_announcements.len());
    for (index, raw) in raw_announcements.iter().enumerate() {
        let announcement_label = format!("{label} announcements[{index}]");
        let announcement = validate_peer_source_announcement_value(raw, &announcement_label)?;
        let key = format!("{}\n{}", announcement.model_id, announcement.peer_base_url);
        if !seen.insert(key) {
            return Err(format!(
                "{announcement_label} duplicated model_id and peer_base_url in feed"
            ));
        }
        announcements.push((announcement_label, announcement));
    }
    Ok(announcements)
}

fn apply_peer_source_announcements_to_catalog_value(
    label: &str,
    mut value: Value,
    announcements: &[(String, PeerSourceAnnouncement)],
    output_label: &str,
) -> Result<(Value, String, usize, usize), String> {
    if announcements.is_empty() {
        return Err("at least one peer source announcement is required".to_string());
    }
    let summary = validate_catalog_entry_value(&value, label)?;
    let artifact_byte_count = json_required_usize(&value, "artifact_byte_count", label)?;
    let artifact_sha256 =
        normalize_sha256(&json_required_string(&value, "artifact_sha256", label)?)?;
    let peer_offer_sha256 =
        normalize_sha256(&json_required_string(&value, "peer_offer_sha256", label)?)?;
    let peer_offer_merkle_root_sha256 = normalize_sha256(&json_required_string(
        &value,
        "peer_offer_merkle_root_sha256",
        label,
    )?)?;
    let peer_offer_route = json_required_string(&value, "peer_offer_route", label)?;
    let peer_offer_chunk_count = json_required_usize(&value, "peer_offer_chunk_count", label)?;

    let mut applied = Vec::with_capacity(announcements.len());
    for (announcement_label, announcement) in announcements {
        if announcement.model_id != summary.model_id {
            return Err(format!(
                "{announcement_label} model_id {} did not match catalog model_id {}",
                announcement.model_id, summary.model_id
            ));
        }
        if announcement.artifact_byte_count != artifact_byte_count {
            return Err(format!(
                "{announcement_label} artifact_byte_count {} did not match catalog artifact_byte_count {}",
                announcement.artifact_byte_count, artifact_byte_count
            ));
        }
        if announcement.peer_offer_route != peer_offer_route {
            return Err(format!(
                "{announcement_label} peer_offer_route {} did not match catalog peer_offer_route {}",
                announcement.peer_offer_route, peer_offer_route
            ));
        }
        if announcement.peer_offer_chunk_count != peer_offer_chunk_count {
            return Err(format!(
                "{announcement_label} peer_offer_chunk_count {} did not match catalog peer_offer_chunk_count {}",
                announcement.peer_offer_chunk_count, peer_offer_chunk_count
            ));
        }
        require_normalized_equal(
            "artifact_sha256",
            &artifact_sha256,
            &announcement.artifact_sha256,
            announcement_label,
        )?;
        require_normalized_equal(
            "peer_offer_sha256",
            &peer_offer_sha256,
            &announcement.peer_offer_sha256,
            announcement_label,
        )?;
        require_normalized_equal(
            "peer_offer_merkle_root_sha256",
            &peer_offer_merkle_root_sha256,
            &announcement.peer_offer_merkle_root_sha256,
            announcement_label,
        )?;
        applied.push(announcement.clone());
    }

    let object = value
        .as_object_mut()
        .ok_or_else(|| format!("{label} catalog entry must be a JSON object"))?;
    let mut peer_sources = match object.remove("peer_sources") {
        Some(Value::Array(values)) => values,
        Some(_) => return Err(format!("{label} peer_sources must be an array")),
        None => Vec::new(),
    };
    let mut seen = BTreeSet::new();
    for (index, peer_source) in peer_sources.iter().enumerate() {
        let peer_label = format!("{label} peer_sources[{index}]");
        seen.insert(validate_catalog_peer_source_value(
            peer_source,
            &peer_label,
        )?);
    }
    let mut applied_count = 0usize;
    for announcement in applied {
        if seen.insert(announcement.peer_base_url.clone()) {
            peer_sources.push(announcement.peer_source);
            applied_count += 1;
        }
    }
    object.insert(
        "peer_source_count".to_string(),
        Value::Number((peer_sources.len() as u64).into()),
    );
    object.insert("peer_sources".to_string(), Value::Array(peer_sources));

    let updated_summary = validate_catalog_entry_value(&value, output_label)?;
    Ok((
        value,
        updated_summary.model_id,
        applied_count,
        updated_summary.peer_source_count,
    ))
}

fn validate_peer_source_announcement_value(
    value: &Value,
    label: &str,
) -> Result<PeerSourceAnnouncement, String> {
    require_json_u64_field(value, "announcement_version", label, 0)?;
    require_json_string_field(
        value,
        "announcement_kind",
        label,
        "minimodel.peer_source_announcement",
    )?;
    require_json_string_field(
        value,
        "announcement_status",
        label,
        "peer-source-available-metadata-only",
    )?;
    require_json_string_field(value, "artifact_kind", label, "slm")?;
    require_json_bool_false(value, "project_server_serves_model_bytes", label)?;
    require_json_bool_false(value, "project_server_runs_model_bytes", label)?;
    require_json_bool_false(value, "project_server_proxies_model_bytes", label)?;

    let model_id = validate_minimodel_model_id(&json_required_string(value, "model_id", label)?)?;
    let artifact_byte_count = json_required_usize(value, "artifact_byte_count", label)?;
    if artifact_byte_count == 0 {
        return Err(format!("{label} artifact_byte_count must be positive"));
    }
    let artifact_sha256 =
        normalize_sha256(&json_required_string(value, "artifact_sha256", label)?)?;
    let peer_offer_sha256 =
        normalize_sha256(&json_required_string(value, "peer_offer_sha256", label)?)?;
    let peer_offer_merkle_root_sha256 = normalize_sha256(&json_required_string(
        value,
        "peer_offer_merkle_root_sha256",
        label,
    )?)?;
    let peer_offer_route = json_required_string(value, "peer_offer_route", label)?;
    if peer_offer_route.contains('\n') || peer_offer_route.contains('\r') {
        return Err(format!(
            "{label} peer_offer_route must not contain newlines"
        ));
    }
    let peer_offer_chunk_count = json_required_usize(value, "peer_offer_chunk_count", label)?;
    if peer_offer_chunk_count == 0 {
        return Err(format!("{label} peer_offer_chunk_count must be positive"));
    }
    normalize_sha256(&json_required_string(value, "catalog_entry_sha256", label)?)?;
    let peer_source = value
        .get("peer_source")
        .ok_or_else(|| format!("{label} missing object peer_source"))?;
    let peer_base_url =
        validate_catalog_peer_source_value(peer_source, &format!("{label} peer_source"))?;

    Ok(PeerSourceAnnouncement {
        model_id,
        artifact_byte_count,
        artifact_sha256,
        peer_offer_route,
        peer_offer_chunk_count,
        peer_offer_sha256,
        peer_offer_merkle_root_sha256,
        peer_source: known_peer_source_value(&peer_base_url),
        peer_base_url,
    })
}

fn require_json_string_field(
    value: &Value,
    key: &str,
    label: &str,
    expected: &str,
) -> Result<(), String> {
    let actual = json_required_string(value, key, label)?;
    if actual == expected {
        return Ok(());
    }
    Err(format!("{label} {key} must be {expected}, got {actual}"))
}

fn require_json_u64_field(
    value: &Value,
    key: &str,
    label: &str,
    expected: u64,
) -> Result<(), String> {
    let actual = json_required_usize(value, key, label)? as u64;
    if actual == expected {
        return Ok(());
    }
    Err(format!("{label} {key} must be {expected}, got {actual}"))
}

fn require_json_bool_false(value: &Value, key: &str, label: &str) -> Result<(), String> {
    match value.get(key).and_then(Value::as_bool) {
        Some(false) => Ok(()),
        Some(true) => Err(format!("{label} {key} must be false")),
        None => Err(format!("{label} missing false {key}")),
    }
}

fn require_json_bool_true(value: &Value, key: &str, label: &str) -> Result<(), String> {
    match value.get(key).and_then(Value::as_bool) {
        Some(true) => Ok(()),
        Some(false) => Err(format!("{label} {key} must be true")),
        None => Err(format!("{label} missing true {key}")),
    }
}

fn json_required_string(value: &Value, key: &str, label: &str) -> Result<String, String> {
    let text = value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("{label} missing string {key}"))?;
    Ok(text.to_string())
}

fn json_required_object<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a Value, String> {
    let object = value
        .get(key)
        .filter(|value| value.is_object())
        .ok_or_else(|| format!("{label} missing object {key}"))?;
    Ok(object)
}

fn json_required_array<'a>(
    value: &'a Value,
    key: &str,
    label: &str,
) -> Result<&'a [Value], String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{label} missing array {key}"))
}

fn json_required_string_array(
    value: &Value,
    key: &str,
    label: &str,
) -> Result<Vec<String>, String> {
    let array = json_required_array(value, key, label)?;
    let mut strings = Vec::with_capacity(array.len());
    for (index, item) in array.iter().enumerate() {
        let Some(text) = item.as_str().map(str::trim).filter(|text| !text.is_empty()) else {
            return Err(format!("{label} {key}[{index}] must be a non-empty string"));
        };
        strings.push(text.to_string());
    }
    Ok(strings)
}

fn json_required_bool(value: &Value, key: &str, label: &str) -> Result<bool, String> {
    value
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{label} missing boolean {key}"))
}

fn json_optional_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn json_required_usize(value: &Value, key: &str, label: &str) -> Result<usize, String> {
    json_optional_usize(value, key, label)?.ok_or_else(|| format!("{label} missing integer {key}"))
}

fn json_optional_usize(value: &Value, key: &str, label: &str) -> Result<Option<usize>, String> {
    let Some(raw) = value.get(key) else {
        return Ok(None);
    };
    let Some(number) = raw.as_u64() else {
        return Err(format!("{label} {key} must be a non-negative integer"));
    };
    usize::try_from(number)
        .map(Some)
        .map_err(|_| format!("{label} {key} is too large for this platform"))
}

fn validate_peer_companion_state_value(
    value: &Value,
    expected_model_id: &str,
    share: &PeerPieceServe,
    piece_set_sha256: &str,
) -> Result<String, String> {
    let label = "peer companion state";
    require_json_string_field(value, "state_kind", label, "minimodel.peer_companion_state")?;
    require_json_u64_field(value, "state_version", label, 0)?;
    require_json_string_field(value, "model_id", label, expected_model_id)?;
    require_json_string_field(value, "share_status", label, "ready-to-serve")?;
    require_json_string_field(
        value,
        "transfer_protocol",
        label,
        "direct-http-piece-server-v0",
    )?;
    require_json_string_field(
        value,
        "consent_status",
        label,
        "operator-reviewed-consenting-peer",
    )?;
    let peer_base_url =
        normalize_known_peer_base_url(&json_required_string(value, "peer_base_url", label)?)?;

    let manifest = json_required_object(value, "manifest", label)?;
    let manifest_sha = normalize_sha256(&json_required_string(
        manifest,
        "sha256",
        "peer companion state manifest",
    )?)?;
    if manifest_sha != share.manifest_sha256 {
        return Err(format!(
            "peer companion state manifest.sha256 mismatch: expected {}, got {manifest_sha}",
            share.manifest_sha256
        ));
    }

    let artifact = json_required_object(value, "artifact", label)?;
    let artifact_sha = normalize_sha256(&json_required_string(
        artifact,
        "sha256",
        "peer companion state artifact",
    )?)?;
    if artifact_sha != share.artifact_sha256 {
        return Err(format!(
            "peer companion state artifact.sha256 mismatch: expected {}, got {artifact_sha}",
            share.artifact_sha256
        ));
    }

    let pieces = json_required_object(value, "pieces", label)?;
    require_json_string_field(
        pieces,
        "piece_set_path_hint",
        "peer companion state pieces",
        "minimodel.peer_piece_set.v0",
    )?;
    let state_piece_set_sha = normalize_sha256(&json_required_string(
        pieces,
        "piece_set_sha256",
        "peer companion state pieces",
    )?)?;
    if state_piece_set_sha != piece_set_sha256 {
        return Err(format!(
            "peer companion state pieces.piece_set_sha256 mismatch: expected {piece_set_sha256}, got {state_piece_set_sha}"
        ));
    }
    let state_piece_count =
        json_required_usize(pieces, "piece_count", "peer companion state pieces")?;
    if state_piece_count != share.piece_count {
        return Err(format!(
            "peer companion state pieces.piece_count mismatch: expected {}, got {state_piece_count}",
            share.piece_count
        ));
    }
    let state_merkle_root = normalize_sha256(&json_required_string(
        pieces,
        "merkle_root_sha256",
        "peer companion state pieces",
    )?)?;
    if state_merkle_root != share.merkle_root_sha256 {
        return Err(format!(
            "peer companion state pieces.merkle_root_sha256 mismatch: expected {}, got {state_merkle_root}",
            share.merkle_root_sha256
        ));
    }

    let boundary = json_required_object(value, "boundary", label)?;
    require_json_bool_false(
        boundary,
        "peer_piece_bytes_in_state",
        "peer companion boundary",
    )?;
    require_json_bool_false(
        boundary,
        "project_server_serves_model_bytes",
        "peer companion boundary",
    )?;
    require_json_bool_false(
        boundary,
        "project_server_runs_model_bytes",
        "peer companion boundary",
    )?;
    require_json_bool_false(
        boundary,
        "project_server_proxies_model_bytes",
        "peer companion boundary",
    )?;

    Ok(peer_base_url)
}

fn expected_peer_status_share(
    manifest: &Manifest,
    manifest_bytes: &[u8],
    piece_set_path: String,
) -> Result<PeerPieceServe, String> {
    let chunks = chunk_proofs_from_manifest(manifest)?;
    let manifest_sha256 = sha256_prefixed(manifest_bytes);
    let model_id = manifest.require("model.id")?.to_string();
    let artifact_byte_count = manifest
        .require("artifact.byte_count")?
        .parse::<u64>()
        .map_err(|_| "artifact.byte_count must be a decimal integer".to_string())?;
    let artifact_sha256 = normalize_sha256(manifest.require("artifact.sha256")?)?;
    let piece_set_text = peer_piece_set_contract_text(
        manifest,
        &manifest_sha256,
        &model_id,
        artifact_byte_count,
        &artifact_sha256,
        &chunks,
    )?;
    Ok(PeerPieceServe {
        piece_set_path,
        piece_set_sha256: sha256_prefixed(piece_set_text.as_bytes()),
        model_id,
        artifact_byte_count,
        piece_count: chunks.len(),
        manifest_sha256,
        artifact_sha256,
        merkle_root_sha256: normalize_sha256(manifest.require("chunks.merkle_root_sha256")?)?,
    })
}

fn peer_status_value(
    share: &PeerPieceServe,
    peer_base_url: &str,
    companion_state_route: Option<&str>,
) -> Result<Value, String> {
    let peer_base_url = normalize_known_peer_base_url(peer_base_url)?;
    Ok(serde_json::json!({
        "status_kind": "minimodel.peer_status",
        "status_version": 0,
        "created_utc": utc_now()?,
        "share_status": "ready-to-serve",
        "transfer_protocol": "direct-http-piece-server-v0",
        "peer_base_url": peer_base_url,
        "model_id": share.model_id,
        "routes": {
            "peer_status": PEER_STATUS_ROUTE,
            "peer_piece_set": "minimodel.peer_piece_set.v0",
            "peer_companion_state": companion_state_route.unwrap_or("not-advertised"),
        },
        "manifest": {
            "sha256": share.manifest_sha256,
        },
        "artifact": {
            "kind": "slm",
            "byte_count": share.artifact_byte_count,
            "sha256": share.artifact_sha256,
        },
        "pieces": {
            "piece_set_path_hint": "minimodel.peer_piece_set.v0",
            "piece_set_sha256": share.piece_set_sha256,
            "piece_count": share.piece_count,
            "merkle_root_sha256": share.merkle_root_sha256,
        },
        "boundary": {
            "metadata_only": true,
            "peer_status_embeds_model_bytes": false,
            "peer_status_embeds_piece_bytes": false,
            "project_server_serves_model_bytes": false,
            "project_server_runs_model_bytes": false,
            "project_server_proxies_model_bytes": false,
            "requires_rust_reverify_before_browser_import": true,
        }
    }))
}

fn peer_status_bytes(
    share: &PeerPieceServe,
    peer_base_url: &str,
    companion_state_route: Option<&str>,
) -> Result<Vec<u8>, String> {
    let status = peer_status_value(share, peer_base_url, companion_state_route)?;
    let text = serde_json::to_string_pretty(&status)
        .map_err(|error| format!("failed to encode peer status JSON: {error}"))?
        + "\n";
    Ok(text.into_bytes())
}

fn validate_peer_status_value(
    value: &Value,
    expected: &PeerPieceServe,
    expected_peer_base_url: &str,
) -> Result<String, String> {
    let label = "peer status";
    require_json_string_field(value, "status_kind", label, "minimodel.peer_status")?;
    require_json_u64_field(value, "status_version", label, 0)?;
    require_json_string_field(value, "share_status", label, "ready-to-serve")?;
    require_json_string_field(
        value,
        "transfer_protocol",
        label,
        "direct-http-piece-server-v0",
    )?;
    require_json_string_field(value, "model_id", label, &expected.model_id)?;
    let peer_base_url =
        normalize_known_peer_base_url(&json_required_string(value, "peer_base_url", label)?)?;
    require_normalized_equal(
        "peer status peer_base_url",
        expected_peer_base_url,
        &peer_base_url,
        "status",
    )?;

    let routes = json_required_object(value, "routes", label)?;
    require_json_string_field(
        routes,
        "peer_status",
        "peer status routes",
        PEER_STATUS_ROUTE,
    )?;
    require_json_string_field(
        routes,
        "peer_piece_set",
        "peer status routes",
        "minimodel.peer_piece_set.v0",
    )?;
    let companion_state_route =
        json_required_string(routes, "peer_companion_state", "peer status routes")?;
    if companion_state_route != "not-advertised"
        && companion_state_route != PEER_COMPANION_STATE_ROUTE
    {
        return Err(format!(
            "peer status routes peer_companion_state must be not-advertised or {PEER_COMPANION_STATE_ROUTE}"
        ));
    }

    let manifest = json_required_object(value, "manifest", label)?;
    let manifest_sha = normalize_sha256(&json_required_string(
        manifest,
        "sha256",
        "peer status manifest",
    )?)?;
    if manifest_sha != expected.manifest_sha256 {
        return Err(format!(
            "peer status manifest.sha256 mismatch: expected {}, got {manifest_sha}",
            expected.manifest_sha256
        ));
    }

    let artifact = json_required_object(value, "artifact", label)?;
    require_json_string_field(artifact, "kind", "peer status artifact", "slm")?;
    let artifact_byte_count = json_required_usize(artifact, "byte_count", "peer status artifact")?;
    if artifact_byte_count as u64 != expected.artifact_byte_count {
        return Err(format!(
            "peer status artifact.byte_count mismatch: expected {}, got {artifact_byte_count}",
            expected.artifact_byte_count
        ));
    }
    let artifact_sha = normalize_sha256(&json_required_string(
        artifact,
        "sha256",
        "peer status artifact",
    )?)?;
    if artifact_sha != expected.artifact_sha256 {
        return Err(format!(
            "peer status artifact.sha256 mismatch: expected {}, got {artifact_sha}",
            expected.artifact_sha256
        ));
    }

    let pieces = json_required_object(value, "pieces", label)?;
    require_json_string_field(
        pieces,
        "piece_set_path_hint",
        "peer status pieces",
        "minimodel.peer_piece_set.v0",
    )?;
    let piece_set_sha = normalize_sha256(&json_required_string(
        pieces,
        "piece_set_sha256",
        "peer status pieces",
    )?)?;
    if piece_set_sha != expected.piece_set_sha256 {
        return Err(format!(
            "peer status pieces.piece_set_sha256 mismatch: expected {}, got {piece_set_sha}",
            expected.piece_set_sha256
        ));
    }
    let piece_count = json_required_usize(pieces, "piece_count", "peer status pieces")?;
    if piece_count != expected.piece_count {
        return Err(format!(
            "peer status pieces.piece_count mismatch: expected {}, got {piece_count}",
            expected.piece_count
        ));
    }
    let merkle_root = normalize_sha256(&json_required_string(
        pieces,
        "merkle_root_sha256",
        "peer status pieces",
    )?)?;
    if merkle_root != expected.merkle_root_sha256 {
        return Err(format!(
            "peer status pieces.merkle_root_sha256 mismatch: expected {}, got {merkle_root}",
            expected.merkle_root_sha256
        ));
    }

    let boundary = json_required_object(value, "boundary", label)?;
    require_json_bool_true(boundary, "metadata_only", "peer status boundary")?;
    require_json_bool_false(
        boundary,
        "peer_status_embeds_model_bytes",
        "peer status boundary",
    )?;
    require_json_bool_false(
        boundary,
        "peer_status_embeds_piece_bytes",
        "peer status boundary",
    )?;
    require_json_bool_false(
        boundary,
        "project_server_serves_model_bytes",
        "peer status boundary",
    )?;
    require_json_bool_false(
        boundary,
        "project_server_runs_model_bytes",
        "peer status boundary",
    )?;
    require_json_bool_false(
        boundary,
        "project_server_proxies_model_bytes",
        "peer status boundary",
    )?;
    require_json_bool_true(
        boundary,
        "requires_rust_reverify_before_browser_import",
        "peer status boundary",
    )?;

    Ok(companion_state_route)
}

fn peer_status_check_value(
    share: &PeerPieceServe,
    peer_base_url: &str,
    status_url: &str,
    companion_state_route: &str,
) -> Result<Value, String> {
    Ok(serde_json::json!({
        "check_kind": "minimodel.peer_status_check",
        "check_version": 0,
        "checked_utc": utc_now()?,
        "status": "validated",
        "peer_base_url": peer_base_url,
        "status_url": status_url,
        "model_id": share.model_id,
        "routes": {
            "peer_status": PEER_STATUS_ROUTE,
            "peer_piece_set": "minimodel.peer_piece_set.v0",
            "peer_companion_state": companion_state_route,
        },
        "manifest": {
            "sha256": share.manifest_sha256,
        },
        "artifact": {
            "kind": "slm",
            "byte_count": share.artifact_byte_count,
            "sha256": share.artifact_sha256,
        },
        "pieces": {
            "piece_set_sha256": share.piece_set_sha256,
            "piece_count": share.piece_count,
            "merkle_root_sha256": share.merkle_root_sha256,
        },
        "boundary": {
            "metadata_only": true,
            "peer_piece_bytes_fetched": false,
            "artifact_bytes_fetched": false,
            "project_server_serves_model_bytes": false,
            "project_server_runs_model_bytes": false,
            "project_server_proxies_model_bytes": false,
            "requires_rust_reverify_before_browser_import": true,
        }
    }))
}

fn validate_peer_contact_card_value(
    value: &Value,
    label: &str,
) -> Result<(String, String), String> {
    require_json_string_field(value, "card_kind", label, "minimodel.peer_contact_card")?;
    require_json_u64_field(value, "card_version", label, 0)?;
    let peer_label =
        validate_peer_contact_label(&json_required_string(value, "peer_label", label)?)?;
    require_json_string_field(
        value,
        "transfer_protocol",
        label,
        "direct-http-piece-server-v0",
    )?;
    require_json_string_field(
        value,
        "contact_status",
        label,
        "operator-reviewed-consenting-peer",
    )?;
    require_json_string_field(
        value,
        "intended_use",
        label,
        "known-peer-url-for-minimodel-peer-piece-import",
    )?;
    let peer_base_url =
        normalize_known_peer_base_url(&json_required_string(value, "peer_base_url", label)?)?;
    require_json_bool_false(value, "project_server_serves_model_bytes", label)?;
    require_json_bool_false(value, "project_server_runs_model_bytes", label)?;
    require_json_bool_false(value, "project_server_proxies_model_bytes", label)?;

    let boundary = json_required_object(value, "boundary", label)?;
    require_json_bool_true(boundary, "metadata_only", "peer contact card boundary")?;
    require_json_bool_false(
        boundary,
        "peer_contact_card_embeds_model_bytes",
        "peer contact card boundary",
    )?;
    require_json_bool_false(
        boundary,
        "project_server_serves_model_bytes",
        "peer contact card boundary",
    )?;
    require_json_bool_false(
        boundary,
        "project_server_runs_model_bytes",
        "peer contact card boundary",
    )?;
    require_json_bool_false(
        boundary,
        "project_server_proxies_model_bytes",
        "peer contact card boundary",
    )?;
    require_json_bool_true(
        boundary,
        "requires_rust_reverify_before_browser_import",
        "peer contact card boundary",
    )?;

    Ok((peer_label, peer_base_url))
}

fn validate_peer_contact_label(value: &str) -> Result<String, String> {
    let label = value.trim();
    if label.is_empty()
        || label.len() > 96
        || !label.is_ascii()
        || label.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err("peer contact label must be 1-96 printable ASCII bytes".to_string());
    }
    Ok(label.to_string())
}

fn default_peer_contact_label(peer_base_url: &str) -> Result<String, String> {
    let label = peer_base_url
        .strip_prefix("http://")
        .unwrap_or(peer_base_url)
        .trim_end_matches('/');
    validate_peer_contact_label(label)
}

fn require_peer_offer_equal(
    peer_offer: &Manifest,
    key: &str,
    expected: &str,
) -> Result<(), String> {
    let actual = peer_offer.require(key)?;
    if actual.eq_ignore_ascii_case(expected) {
        return Ok(());
    }
    Err(format!(
        "peer offer {key} mismatch: expected {expected}, got {actual}"
    ))
}

fn require_normalized_equal(
    field: &str,
    expected: &str,
    actual: &str,
    actual_label: &str,
) -> Result<(), String> {
    if expected.eq_ignore_ascii_case(actual) {
        return Ok(());
    }
    Err(format!(
        "{field} mismatch: expected {expected}, {actual_label} had {actual}"
    ))
}

fn require_value_member(value: &str, key: &str, allowed: &[&str]) -> Result<(), String> {
    if allowed.contains(&value) {
        return Ok(());
    }
    Err(format!("{key} has unsupported value {value}"))
}

fn file_name_for_route(path: &Path, label: &str) -> Result<String, String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("{label} path must include a file name"))?;
    require_safe_piece_file_name(file_name)?;
    Ok(file_name.to_string())
}

fn validate_huggingface_model_id(value: &str) -> Result<String, String> {
    let text = value.trim();
    let bytes = text.as_bytes();
    if bytes.len() < 2
        || bytes.len() > 180
        || !bytes[0].is_ascii_alphanumeric()
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
    {
        return Err("source Hugging Face model id must be route-safe ASCII".to_string());
    }
    Ok(text.to_string())
}

fn validate_minimodel_model_id(value: &str) -> Result<String, String> {
    let text = value.trim();
    if text.is_empty()
        || !text.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b'.' | b'/')
        })
    {
        return Err("model.id must be route-safe lowercase ASCII".to_string());
    }
    Ok(text.to_string())
}

fn normalize_known_peer_base_url(value: &str) -> Result<String, String> {
    let url = HttpUrl::parse(value)?;
    reject_project_server_peer_host(&url.host)?;
    let mut text = url.to_string();
    if !text.ends_with('/') {
        text.push('/');
    }
    Ok(text)
}

fn artifact_file_name_for_share(path: &Path) -> Result<String, String> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "artifact path must include a file name".to_string())?;
    require_safe_piece_file_name(file_name)?;
    if !file_name.ends_with(".slm") {
        return Err("artifact file name must end with .slm".to_string());
    }
    Ok(file_name.to_string())
}

fn local_file_hint(path: &Path, label: &str) -> Result<String, String> {
    let hint = path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{label} path must include a local name"))?;
    if hint.contains('/') || hint.contains('\\') || hint.contains("..") || !hint.is_ascii() {
        return Err(format!("{label} path hint must be safe ASCII"));
    }
    Ok(hint.to_string())
}

fn artifact_stem_for_share(file_name: &str) -> Result<String, String> {
    let stem = file_name
        .strip_suffix(".slm")
        .ok_or_else(|| "artifact file name must end with .slm".to_string())?;
    if stem.is_empty() {
        return Err("artifact file name stem must not be empty".to_string());
    }
    require_safe_piece_file_name(stem)?;
    Ok(stem.to_string())
}

fn known_peer_sources_json(peer_base_url: &str) -> String {
    let fields = known_peer_source_json_fields(peer_base_url);
    format!("[{{{}}}]", fields.join(", "))
}

fn known_peer_source_value(peer_base_url: &str) -> Value {
    let mut object = Map::new();
    object.insert(
        "source_kind".to_string(),
        Value::String("known-direct-http-peer-v0".to_string()),
    );
    object.insert(
        "transport".to_string(),
        Value::String("direct-http-piece-server-v0".to_string()),
    );
    object.insert(
        "peer_base_url".to_string(),
        Value::String(peer_base_url.to_string()),
    );
    object.insert(
        "consent_status".to_string(),
        Value::String("publisher-declared-consenting-peer".to_string()),
    );
    object.insert(
        "project_server_serves_model_bytes".to_string(),
        Value::Bool(false),
    );
    object.insert(
        "project_server_runs_model_bytes".to_string(),
        Value::Bool(false),
    );
    Value::Object(object)
}

fn known_peer_source_json_fields(peer_base_url: &str) -> [String; 6] {
    let fields = [
        json_string_field("source_kind", "known-direct-http-peer-v0"),
        json_string_field("transport", "direct-http-piece-server-v0"),
        json_string_field("peer_base_url", peer_base_url),
        json_string_field("consent_status", "publisher-declared-consenting-peer"),
        json_bool_field("project_server_serves_model_bytes", false),
        json_bool_field("project_server_runs_model_bytes", false),
    ];
    fields
}

fn minimodel_manifest_text(
    model_id: &str,
    artifact_bytes: &[u8],
    slm: &SlmReport,
    chunks: &[ChunkProof],
    peer_offer_route: &str,
    peer_offer_sha256: &str,
    source_huggingface_model_id: Option<&str>,
) -> Result<String, String> {
    let source_kind = if source_huggingface_model_id.is_some() {
        "safetensors"
    } else {
        "slm-native"
    };
    let source_id = source_huggingface_model_id.unwrap_or(model_id);
    let source_discovery_kind = if source_huggingface_model_id.is_some() {
        "huggingface-hub"
    } else {
        "local-file"
    };
    let chunk_list_sha256 = sha256_prefixed(chunk_list_text(chunks).as_bytes());
    let mut text = String::new();
    push_line(&mut text, "manifest.version", "0");
    push_line(&mut text, "manifest.kind", "minimodel.manifest");
    push_line(&mut text, "manifest.schema_id", "minimodel.manifest.v0");
    push_line(
        &mut text,
        "manifest.schema_checksum",
        MINIMODEL_MANIFEST_SCHEMA_CHECKSUM,
    );
    push_line(&mut text, "manifest.created_utc", &utc_now()?);
    push_line(&mut text, "model.id", model_id);
    push_line(&mut text, "model.version", "0.1.0-local");
    push_line(&mut text, "publisher.id", "local-user");
    push_line(&mut text, "publisher.key_id", "local-user-unsigned");
    push_line(&mut text, "model_card.route", "none");
    push_line(&mut text, "license.route", "none");
    push_line(&mut text, "artifact.kind", "slm");
    push_line(
        &mut text,
        "artifact.byte_count",
        &artifact_bytes.len().to_string(),
    );
    push_line(
        &mut text,
        "artifact.sha256",
        &sha256_prefixed(artifact_bytes),
    );
    push_line(
        &mut text,
        "artifact.acquisition",
        if source_huggingface_model_id.is_some() {
            "user-external-download"
        } else {
            "user-local-file"
        },
    );
    push_line(&mut text, "artifact.project_server_url", "none");
    push_line(
        &mut text,
        "slm.format_version",
        &slm.format_version.to_string(),
    );
    push_line(&mut text, "slm.checksum", &slm.checksum);
    push_line(&mut text, "slm.model_shape", &slm.model_shape);
    push_line(&mut text, "slm.quantization", &slm.quantization);
    push_line(&mut text, "slm.tokenizer_checksum", &slm.tokenizer_checksum);
    push_line(
        &mut text,
        "slm.tensor_layout_checksum",
        &slm.tensor_layout_checksum,
    );
    push_line(&mut text, "runtime.compatibility", "tinyrustlm");
    push_line(&mut text, "runtime.minimum_version", "0.1.0");
    push_line(&mut text, "source.kind", source_kind);
    push_line(&mut text, "source.id", source_id);
    push_line(&mut text, "source.revision", "local-conversion");
    push_line(&mut text, "source.discovery.kind", source_discovery_kind);
    push_line(&mut text, "source.discovery.route", "none");
    push_line(&mut text, "source.discovery.revision", "unknown");
    push_line(&mut text, "source.discovery.user_token_required", "false");
    if let Some(source_id) = source_huggingface_model_id {
        push_line(&mut text, "source.huggingface_model_id", source_id);
    }
    push_line(&mut text, "evidence.admission.status", "passed");
    push_line(&mut text, "evidence.admission.route", peer_offer_route);
    push_line(&mut text, "evidence.admission.sha256", peer_offer_sha256);
    push_line(&mut text, "chunks.mode", "fixed-size-merkle-v0");
    push_line(
        &mut text,
        "chunks.size",
        &DEFAULT_PEER_CHUNK_SIZE.to_string(),
    );
    push_line(&mut text, "chunks.count", &chunks.len().to_string());
    push_line(
        &mut text,
        "chunks.merkle_root_sha256",
        &merkle_root_sha256(chunks)?,
    );
    push_line(&mut text, "chunks.list.route", peer_offer_route);
    push_line(&mut text, "chunks.list.sha256", &chunk_list_sha256);
    for (index, chunk) in chunks.iter().enumerate() {
        let prefix = format!("chunk.{index}");
        push_line(
            &mut text,
            &format!("{prefix}.offset"),
            &chunk.offset.to_string(),
        );
        push_line(
            &mut text,
            &format!("{prefix}.byte_count"),
            &chunk.byte_count.to_string(),
        );
        push_line(&mut text, &format!("{prefix}.sha256"), &chunk.sha256);
    }
    push_line(&mut text, "signature.kind", "unsigned-draft");
    push_line(&mut text, "signature.key_id", "none");
    push_line(&mut text, "signature.public_key_route", "none");
    push_line(&mut text, "signature.payload_sha256", "none");
    push_line(&mut text, "signature.value", "none");
    Ok(text)
}

fn peer_offer_text(
    artifact_bytes: &[u8],
    slm: &SlmReport,
    artifact_file_name: &str,
    chunks: &[ChunkProof],
) -> Result<String, String> {
    require_safe_piece_file_name(artifact_file_name)?;
    let mut text = String::new();
    push_line(&mut text, "minimodel_peer_offer_version", "0");
    push_line(&mut text, "offer_kind", "minimodel.peer_offer.v0");
    push_line(&mut text, "artifact_kind", "slm");
    push_line(&mut text, "artifact_file_name", artifact_file_name);
    push_line(
        &mut text,
        "artifact_byte_count",
        &artifact_bytes.len().to_string(),
    );
    push_line(
        &mut text,
        "artifact_sha256",
        &sha256_prefixed(artifact_bytes),
    );
    push_line(&mut text, "slm_version", &slm.format_version.to_string());
    push_line(&mut text, "slm_checksum", &slm.checksum);
    push_line(&mut text, "slm_model_shape", &slm.model_shape);
    push_line(&mut text, "slm_quantization", &slm.quantization);
    push_line(&mut text, "slm_tokenizer_checksum", &slm.tokenizer_checksum);
    push_line(
        &mut text,
        "slm_tensor_layout_checksum",
        &slm.tensor_layout_checksum,
    );
    push_line(&mut text, "chunk_algorithm", "sha256-merkle-v0");
    push_line(
        &mut text,
        "chunk_size",
        &DEFAULT_PEER_CHUNK_SIZE.to_string(),
    );
    push_line(&mut text, "chunk_count", &chunks.len().to_string());
    push_line(
        &mut text,
        "chunk_list_sha256",
        &sha256_prefixed(chunk_list_text(chunks).as_bytes()),
    );
    push_line(
        &mut text,
        "merkle_root_sha256",
        &merkle_root_sha256(chunks)?,
    );
    push_line(&mut text, "transfer_network_available", "false");
    push_line(&mut text, "project_server_serves_model_bytes", "false");
    push_line(&mut text, "project_server_runs_model_bytes", "false");
    push_line(
        &mut text,
        "required_next_gate",
        "consent-based-peer-transfer-and-rust-reverify-before-import",
    );
    for (index, chunk) in chunks.iter().enumerate() {
        let prefix = format!("chunk.{index}");
        push_line(
            &mut text,
            &format!("{prefix}.offset"),
            &chunk.offset.to_string(),
        );
        push_line(
            &mut text,
            &format!("{prefix}.byte_count"),
            &chunk.byte_count.to_string(),
        );
        push_line(&mut text, &format!("{prefix}.sha256"), &chunk.sha256);
    }
    Ok(text)
}

fn json_string_field(key: &str, value: &str) -> String {
    format!("{}: {}", json_string(key), json_string(value))
}

fn json_raw_field(key: &str, value: &str) -> String {
    format!("{}: {}", json_string(key), value)
}

fn json_number_field(key: &str, value: &str) -> String {
    format!("{}: {}", json_string(key), value)
}

fn json_bool_field(key: &str, value: bool) -> String {
    format!(
        "{}: {}",
        json_string(key),
        if value { "true" } else { "false" }
    )
}

fn json_null_field(key: &str) -> String {
    format!("{}: null", json_string(key))
}

fn json_string(value: &str) -> String {
    let mut out = String::new();
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch < ' ' => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn peer_piece_set_text(
    manifest: &Manifest,
    report: &ArtifactVerification,
    chunks: &[ChunkProof],
) -> Result<String, String> {
    peer_piece_set_contract_text(
        manifest,
        &report.manifest_sha256,
        &report.manifest_model_id,
        report.artifact_byte_count,
        &report.artifact_sha256,
        chunks,
    )
}

fn peer_piece_set_contract_text(
    manifest: &Manifest,
    manifest_sha256: &str,
    manifest_model_id: &str,
    artifact_byte_count: u64,
    artifact_sha256: &str,
    chunks: &[ChunkProof],
) -> Result<String, String> {
    let mut text = String::new();
    push_line(&mut text, "piece_set.version", "0");
    push_line(&mut text, "piece_set.kind", "minimodel.peer_piece_set.v0");
    push_line(&mut text, "manifest.sha256", manifest_sha256);
    push_line(&mut text, "manifest.model_id", manifest_model_id);
    push_line(
        &mut text,
        "artifact.byte_count",
        &artifact_byte_count.to_string(),
    );
    push_line(&mut text, "artifact.sha256", artifact_sha256);
    push_line(&mut text, "chunks.mode", "fixed-size-merkle-v0");
    push_line(&mut text, "chunks.size", manifest.require("chunks.size")?);
    push_line(&mut text, "chunks.count", &chunks.len().to_string());
    push_line(
        &mut text,
        "chunks.merkle_root_sha256",
        &merkle_root_sha256(chunks)?,
    );
    push_line(
        &mut text,
        "chunks.list.sha256",
        &sha256_prefixed(chunk_list_text(chunks).as_bytes()),
    );
    push_line(&mut text, "transfer_network_available", "false");
    push_line(&mut text, "project_server_serves_model_bytes", "false");
    push_line(&mut text, "project_server_runs_model_bytes", "false");
    push_line(
        &mut text,
        "required_next_gate",
        "assemble-peer-pieces-and-rust-reverify-before-tinyrustlm-import",
    );
    for (index, chunk) in chunks.iter().enumerate() {
        let prefix = format!("piece.{index}");
        push_line(
            &mut text,
            &format!("{prefix}.file"),
            &piece_file_name(index),
        );
        push_line(
            &mut text,
            &format!("{prefix}.offset"),
            &chunk.offset.to_string(),
        );
        push_line(
            &mut text,
            &format!("{prefix}.byte_count"),
            &chunk.byte_count.to_string(),
        );
        push_line(&mut text, &format!("{prefix}.sha256"), &chunk.sha256);
    }
    Ok(text)
}

fn compare_slm_manifest(manifest: &Manifest, slm: &SlmReport) -> Result<(), String> {
    require_match(
        "slm.format_version",
        manifest.require("slm.format_version")?,
        &slm.format_version.to_string(),
    )?;
    require_match(
        "slm.model_shape",
        manifest.require("slm.model_shape")?,
        &slm.model_shape,
    )?;
    require_match(
        "slm.quantization",
        manifest.require("slm.quantization")?,
        &slm.quantization,
    )?;
    require_match(
        "slm.tokenizer_checksum",
        manifest.require("slm.tokenizer_checksum")?,
        &slm.tokenizer_checksum,
    )?;
    require_match(
        "slm.tensor_layout_checksum",
        manifest.require("slm.tensor_layout_checksum")?,
        &slm.tensor_layout_checksum,
    )?;
    if let Some(expected) = manifest.get("slm.checksum") {
        require_match("slm.checksum", expected, &slm.checksum)?;
    }
    Ok(())
}

fn require_match(field: &str, expected: &str, actual: &str) -> Result<(), String> {
    if expected.eq_ignore_ascii_case(actual) {
        return Ok(());
    }
    Err(format!(
        "{field} mismatch: manifest expected {expected}, artifact had {actual}"
    ))
}

fn require_fixed_chunk_mode(manifest: &Manifest) -> Result<(), String> {
    match manifest.get("chunks.mode") {
        Some("fixed-size-merkle-v0") => Ok(()),
        Some(other) => Err(format!(
            "peer pieces require chunks.mode=fixed-size-merkle-v0, got {other}"
        )),
        None => Err("peer pieces require chunks.mode=fixed-size-merkle-v0".to_string()),
    }
}

fn validate_piece_set(
    manifest: &Manifest,
    manifest_bytes: &[u8],
    piece_set: &Manifest,
) -> Result<(), String> {
    require_piece_set_equal(piece_set, "piece_set.version", "0")?;
    require_piece_set_equal(piece_set, "piece_set.kind", "minimodel.peer_piece_set.v0")?;
    require_piece_set_equal(
        piece_set,
        "manifest.sha256",
        &sha256_prefixed(manifest_bytes),
    )?;
    require_piece_set_equal(
        piece_set,
        "manifest.model_id",
        manifest.require("model.id")?,
    )?;
    require_piece_set_equal(
        piece_set,
        "artifact.byte_count",
        manifest.require("artifact.byte_count")?,
    )?;
    require_piece_set_equal(
        piece_set,
        "artifact.sha256",
        &normalize_sha256(manifest.require("artifact.sha256")?)?,
    )?;
    require_piece_set_equal(piece_set, "chunks.mode", "fixed-size-merkle-v0")?;
    require_piece_set_equal(piece_set, "chunks.size", manifest.require("chunks.size")?)?;
    require_piece_set_equal(piece_set, "chunks.count", manifest.require("chunks.count")?)?;
    require_piece_set_equal(
        piece_set,
        "chunks.merkle_root_sha256",
        &normalize_sha256(manifest.require("chunks.merkle_root_sha256")?)?,
    )?;
    if let Some(expected) = manifest.get("chunks.list.sha256") {
        require_piece_set_equal(
            piece_set,
            "chunks.list.sha256",
            &normalize_sha256(expected)?,
        )?;
    }
    require_piece_set_equal(piece_set, "transfer_network_available", "false")?;
    require_piece_set_equal(piece_set, "project_server_serves_model_bytes", "false")?;
    require_piece_set_equal(piece_set, "project_server_runs_model_bytes", "false")?;
    Ok(())
}

fn validate_optional_peer_companion_state(
    base_url: &HttpUrl,
    piece_set_url: &HttpUrl,
    manifest: &Manifest,
    manifest_bytes: &[u8],
    piece_set: &Manifest,
    piece_set_bytes: &[u8],
) -> Result<PeerCompanionMetadataCheck, String> {
    let state_url = base_url.join_file(PEER_COMPANION_STATE_ROUTE)?;
    let state_url_text = state_url.to_string();
    let state_bytes = match http_get_bytes(&state_url, 64 * 1024) {
        Ok(bytes) => bytes,
        Err(error) if is_peer_http_not_found(&error) => {
            return Ok(PeerCompanionMetadataCheck {
                url: state_url_text,
                status: "not-advertised".to_string(),
            });
        }
        Err(error) => {
            return Err(format!(
                "failed to fetch peer companion state metadata from {state_url_text}: {error}"
            ));
        }
    };
    let state_text = std::str::from_utf8(&state_bytes)
        .map_err(|_| "peer companion state must be valid UTF-8".to_string())?;
    let state_value: Value = serde_json::from_str(state_text)
        .map_err(|error| format!("failed to parse peer companion state JSON: {error}"))?;
    let share = PeerPieceServe {
        piece_set_path: piece_set_url.to_string(),
        piece_set_sha256: sha256_prefixed(piece_set_bytes),
        model_id: manifest.require("model.id")?.to_string(),
        artifact_byte_count: manifest
            .require("artifact.byte_count")?
            .parse::<u64>()
            .map_err(|_| "artifact.byte_count must be a decimal integer".to_string())?,
        piece_count: parse_usize(piece_set.require("chunks.count")?, "chunks.count")?,
        manifest_sha256: sha256_prefixed(manifest_bytes),
        artifact_sha256: normalize_sha256(manifest.require("artifact.sha256")?)?,
        merkle_root_sha256: normalize_sha256(manifest.require("chunks.merkle_root_sha256")?)?,
    };
    let state_peer_base_url = validate_peer_companion_state_value(
        &state_value,
        manifest.require("model.id")?,
        &share,
        &sha256_prefixed(piece_set_bytes),
    )?;
    let expected_peer_base_url = normalize_known_peer_base_url(&base_url.to_string())?;
    require_normalized_equal(
        "peer companion state peer_base_url",
        &expected_peer_base_url,
        &state_peer_base_url,
        "state",
    )?;
    Ok(PeerCompanionMetadataCheck {
        url: state_url_text,
        status: "validated".to_string(),
    })
}

fn is_peer_http_not_found(error: &str) -> bool {
    error.contains("peer HTTP request failed: HTTP/1.0 404 ")
        || error.contains("peer HTTP request failed: HTTP/1.1 404 ")
}

fn require_piece_set_equal(piece_set: &Manifest, key: &str, expected: &str) -> Result<(), String> {
    let actual = piece_set.require(key)?;
    if actual.eq_ignore_ascii_case(expected) {
        return Ok(());
    }
    Err(format!(
        "{key} mismatch: expected {expected}, peer piece set had {actual}"
    ))
}

fn assemble_peer_pieces_to_temp(
    piece_set: &Manifest,
    piece_dir: &Path,
    temp_path: &Path,
) -> Result<(), String> {
    let chunk_count = parse_usize(piece_set.require("chunks.count")?, "chunks.count")?;
    let mut out =
        fs::File::create(temp_path).map_err(|error| format!("failed to create output: {error}"))?;
    let mut written = 0u64;
    for index in 0..chunk_count {
        let prefix = format!("piece.{index}");
        let spec = peer_piece_spec(piece_set, index)?;
        if spec.offset as u64 != written {
            return Err(format!(
                "{prefix}.offset mismatch: expected append offset {written}, got {}",
                spec.offset
            ));
        }
        let bytes = fs::read(piece_dir.join(&spec.file_name))
            .map_err(|error| format!("failed to read peer piece {index}: {error}"))?;
        validate_piece_bytes(&prefix, &bytes, spec.byte_count, &spec.sha256, "file had")?;
        use std::io::Write;
        out.write_all(&bytes)
            .map_err(|error| format!("failed to append peer piece {index}: {error}"))?;
        written = written
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| "assembled artifact byte count overflow".to_string())?;
    }
    out.sync_all()
        .map_err(|error| format!("failed to flush assembled artifact: {error}"))?;
    Ok(())
}

fn fetch_peer_pieces_to_temp(
    base_url: &HttpUrl,
    piece_set: &Manifest,
    chunk_count: usize,
    temp_dir: &Path,
) -> Result<(), String> {
    fs::create_dir_all(temp_dir)
        .map_err(|error| format!("failed to create temporary peer piece directory: {error}"))?;
    for index in 0..chunk_count {
        let prefix = format!("piece.{index}");
        let spec = peer_piece_spec(piece_set, index)?;
        let piece_url = base_url.join_file(&spec.file_name)?;
        let bytes = http_get_bytes(&piece_url, spec.byte_count)?;
        validate_piece_bytes(&prefix, &bytes, spec.byte_count, &spec.sha256, "peer sent")?;
        fs::write(temp_dir.join(&spec.file_name), bytes)
            .map_err(|error| format!("failed to write fetched peer piece {index}: {error}"))?;
    }
    Ok(())
}

fn validate_peer_piece_files(piece_set: &Manifest, piece_dir: &Path) -> Result<usize, String> {
    let chunk_count = parse_usize(piece_set.require("chunks.count")?, "chunks.count")?;
    for index in 0..chunk_count {
        let prefix = format!("piece.{index}");
        let spec = peer_piece_spec(piece_set, index)?;
        let bytes = fs::read(piece_dir.join(&spec.file_name))
            .map_err(|error| format!("failed to read peer piece {index}: {error}"))?;
        validate_piece_bytes(&prefix, &bytes, spec.byte_count, &spec.sha256, "file had")?;
    }
    Ok(chunk_count)
}

#[derive(Debug, Clone)]
struct PeerPieceSpec {
    file_name: String,
    offset: usize,
    byte_count: usize,
    sha256: String,
}

fn peer_piece_spec(piece_set: &Manifest, index: usize) -> Result<PeerPieceSpec, String> {
    let prefix = format!("piece.{index}");
    let file_name = piece_set.require(&format!("{prefix}.file"))?;
    require_safe_piece_file_name(file_name)?;
    Ok(PeerPieceSpec {
        file_name: file_name.to_string(),
        offset: parse_usize(
            piece_set.require(&format!("{prefix}.offset"))?,
            "piece.offset",
        )?,
        byte_count: parse_usize(
            piece_set.require(&format!("{prefix}.byte_count"))?,
            "piece.byte_count",
        )?,
        sha256: normalize_sha256(piece_set.require(&format!("{prefix}.sha256"))?)?,
    })
}

fn validate_piece_bytes(
    prefix: &str,
    bytes: &[u8],
    byte_count: usize,
    expected_sha: &str,
    observed_label: &str,
) -> Result<(), String> {
    if bytes.len() != byte_count {
        return Err(format!(
            "{prefix}.byte_count mismatch: expected {byte_count}, {observed_label} {}",
            bytes.len()
        ));
    }
    let actual_sha = sha256_prefixed(bytes);
    if actual_sha != expected_sha {
        return Err(format!(
            "{prefix}.sha256 mismatch: expected {expected_sha}, {observed_label} {actual_sha}"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
enum ServedPeerExtra {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

#[cfg(test)]
fn serve_peer_piece_dir_http_listener(
    piece_dir: &Path,
    listener: TcpListener,
    request_limit: Option<usize>,
) -> Result<(), String> {
    serve_peer_piece_dir_http_listener_with_extra(
        piece_dir,
        listener,
        request_limit,
        BTreeMap::new(),
        "/",
    )
}

fn serve_peer_pieces_http_listener(
    manifest_path: &Path,
    piece_dir: &Path,
    listener: TcpListener,
    request_limit: Option<usize>,
    peer_base_url: &str,
) -> Result<PeerPieceServe, String> {
    let report = inspect_peer_piece_dir_path(manifest_path, piece_dir)?;
    let mut extra_files = BTreeMap::new();
    extra_files.insert(
        PEER_STATUS_ROUTE.to_string(),
        ServedPeerExtra::Bytes(peer_status_bytes(&report, peer_base_url, None)?),
    );
    let serve_path_prefix = peer_serve_path_prefix(peer_base_url)?;
    serve_peer_piece_dir_http_listener_with_extra(
        piece_dir,
        listener,
        request_limit,
        extra_files,
        &serve_path_prefix,
    )?;
    Ok(report)
}

fn serve_peer_companion_http_listener(
    state_path: &Path,
    manifest_path: &Path,
    piece_dir: &Path,
    listener: TcpListener,
    request_limit: Option<usize>,
) -> Result<PeerCompanionServe, String> {
    let report = inspect_peer_companion_state_path(state_path, manifest_path, piece_dir)?;
    let mut extra_files = BTreeMap::new();
    extra_files.insert(
        PEER_STATUS_ROUTE.to_string(),
        ServedPeerExtra::Bytes(peer_status_bytes(
            &PeerPieceServe {
                piece_set_path: report.piece_set_path.clone(),
                piece_set_sha256: report.piece_set_sha256.clone(),
                model_id: report.model_id.clone(),
                artifact_byte_count: report.artifact_byte_count,
                piece_count: report.piece_count,
                manifest_sha256: report.manifest_sha256.clone(),
                artifact_sha256: report.artifact_sha256.clone(),
                merkle_root_sha256: report.merkle_root_sha256.clone(),
            },
            &report.peer_base_url,
            Some(&report.state_route),
        )?),
    );
    extra_files.insert(
        report.state_route.clone(),
        ServedPeerExtra::Path(state_path.to_path_buf()),
    );
    let serve_path_prefix = peer_serve_path_prefix(&report.peer_base_url)?;
    serve_peer_piece_dir_http_listener_with_extra(
        piece_dir,
        listener,
        request_limit,
        extra_files,
        &serve_path_prefix,
    )?;
    Ok(report)
}

fn serve_peer_piece_dir_http_listener_with_extra(
    piece_dir: &Path,
    listener: TcpListener,
    request_limit: Option<usize>,
    extra_files: BTreeMap<String, ServedPeerExtra>,
    serve_path_prefix: &str,
) -> Result<(), String> {
    let piece_set_text = fs::read_to_string(piece_dir.join("minimodel.peer_piece_set.v0"))
        .map_err(|error| format!("failed to read peer piece set: {error}"))?;
    let piece_set = Manifest::parse(&piece_set_text)?;
    let allowed_names = allowed_peer_piece_names(&piece_set, &extra_files)?;
    let serve_path_prefix = normalize_serve_path_prefix(serve_path_prefix)?;
    let mut served = 0usize;
    loop {
        if let Some(limit) = request_limit {
            if served >= limit {
                return Ok(());
            }
        }
        let (mut stream, _) = listener
            .accept()
            .map_err(|error| format!("failed to accept peer request: {error}"))?;
        served = served
            .checked_add(1)
            .ok_or_else(|| "served request count overflow".to_string())?;
        if let Err(error) = serve_peer_piece_request(
            &mut stream,
            piece_dir,
            &allowed_names,
            &extra_files,
            &serve_path_prefix,
        ) {
            let _ = write_http_response(&mut stream, "400 Bad Request", error.as_bytes());
        }
    }
}

fn allowed_peer_piece_names(
    piece_set: &Manifest,
    extra_files: &BTreeMap<String, ServedPeerExtra>,
) -> Result<BTreeSet<String>, String> {
    let chunk_count = parse_usize(piece_set.require("chunks.count")?, "chunks.count")?;
    let mut names = BTreeSet::new();
    names.insert("minimodel.peer_piece_set.v0".to_string());
    for index in 0..chunk_count {
        names.insert(peer_piece_spec(piece_set, index)?.file_name);
    }
    for file_name in extra_files.keys() {
        require_safe_piece_file_name(file_name)?;
        if !names.insert(file_name.clone()) {
            return Err(format!(
                "extra served peer file {file_name} conflicts with peer-piece output"
            ));
        }
    }
    Ok(names)
}

fn serve_peer_piece_request(
    stream: &mut TcpStream,
    piece_dir: &Path,
    allowed_names: &BTreeSet<String>,
    extra_files: &BTreeMap<String, ServedPeerExtra>,
    serve_path_prefix: &str,
) -> Result<(), String> {
    let request = read_http_request_headers(stream)?;
    let request_text = std::str::from_utf8(&request)
        .map_err(|_| "peer request headers must be valid UTF-8".to_string())?;
    let request_line = request_text
        .lines()
        .next()
        .ok_or_else(|| "peer request missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "peer request missing method".to_string())?;
    let path = parts
        .next()
        .ok_or_else(|| "peer request missing path".to_string())?;
    if method != "GET" {
        write_http_response(stream, "405 Method Not Allowed", b"method not allowed")?;
        return Ok(());
    }
    if path.contains('?') || path.contains('#') {
        write_http_response(stream, "400 Bad Request", b"plain file paths only")?;
        return Ok(());
    }
    let file_name = match file_name_from_served_path(path, serve_path_prefix) {
        Ok(file_name) => file_name,
        Err(PeerPathError::WrongPrefix) => {
            write_http_response(stream, "404 Not Found", b"not found")?;
            return Ok(());
        }
        Err(PeerPathError::BadPath) => {
            write_http_response(stream, "400 Bad Request", b"bad peer piece path")?;
            return Ok(());
        }
    };
    if file_name.is_empty() || require_safe_piece_file_name(file_name).is_err() {
        write_http_response(stream, "400 Bad Request", b"bad peer piece path")?;
        return Ok(());
    }
    if !allowed_names.contains(file_name) {
        write_http_response(stream, "404 Not Found", b"not found")?;
        return Ok(());
    }
    let bytes = match extra_files.get(file_name) {
        Some(ServedPeerExtra::Path(path)) => {
            fs::read(path).map_err(|error| format!("failed to read served peer file: {error}"))?
        }
        Some(ServedPeerExtra::Bytes(bytes)) => bytes.clone(),
        None => fs::read(piece_dir.join(file_name))
            .map_err(|error| format!("failed to read served peer file: {error}"))?,
    };
    write_http_response(stream, "200 OK", &bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PeerPathError {
    WrongPrefix,
    BadPath,
}

fn peer_serve_path_prefix(peer_base_url: &str) -> Result<String, String> {
    let base_url = HttpUrl::parse(peer_base_url)?;
    normalize_serve_path_prefix(&base_url.path)
}

fn normalize_serve_path_prefix(prefix: &str) -> Result<String, String> {
    if prefix.is_empty() || prefix == "/" {
        return Ok("/".to_string());
    }
    if !prefix.starts_with('/') || prefix.contains('?') || prefix.contains('#') {
        return Err("peer serve path prefix must be an absolute plain path".to_string());
    }
    if prefix.contains('\\')
        || prefix.contains("..")
        || prefix.contains('\n')
        || prefix.contains('\r')
    {
        return Err("peer serve path prefix must not contain unsafe path components".to_string());
    }
    Ok(format!("{}/", prefix.trim_end_matches('/')))
}

fn file_name_from_served_path<'a>(
    request_path: &'a str,
    serve_path_prefix: &str,
) -> Result<&'a str, PeerPathError> {
    if !request_path.starts_with('/') || request_path.contains('\\') {
        return Err(PeerPathError::BadPath);
    }
    if serve_path_prefix == "/" {
        let file_name = request_path.trim_start_matches('/');
        if file_name.contains('/') {
            return Err(PeerPathError::BadPath);
        }
        return Ok(file_name);
    }
    let Some(file_name) = request_path.strip_prefix(serve_path_prefix) else {
        return Err(PeerPathError::WrongPrefix);
    };
    if file_name.is_empty() || file_name.contains('/') {
        return Err(PeerPathError::BadPath);
    }
    Ok(file_name)
}

fn read_http_request_headers(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| format!("failed to read peer request: {error}"))?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(request);
        }
        if request.len() > 16 * 1024 {
            return Err("peer request headers exceeded 16 KiB".to_string());
        }
    }
    Err("peer request missing HTTP header terminator".to_string())
}

fn write_http_response(stream: &mut TcpStream, status: &str, body: &[u8]) -> Result<(), String> {
    let header = format!(
        "HTTP/1.0 {status}\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: application/octet-stream\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .map_err(|error| format!("failed to write peer response header: {error}"))?;
    stream
        .write_all(body)
        .map_err(|error| format!("failed to write peer response body: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("failed to flush peer response: {error}"))
}

fn create_empty_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        if !path.is_dir() {
            return Err("peer piece output path exists and is not a directory".to_string());
        }
        let mut entries = fs::read_dir(path)
            .map_err(|error| format!("failed to inspect peer piece output directory: {error}"))?;
        if entries.next().is_some() {
            return Err("peer piece output directory must be empty".to_string());
        }
    } else {
        fs::create_dir_all(path)
            .map_err(|error| format!("failed to create peer piece output directory: {error}"))?;
    }
    Ok(())
}

fn ensure_parent_dir(path: &Path, label: &str) -> Result<(), String> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {label} directory: {error}"))
}

fn piece_file_name(index: usize) -> String {
    format!("piece-{index:08}.bin")
}

fn require_safe_piece_file_name(file_name: &str) -> Result<(), String> {
    if file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains("..")
        || file_name.contains('\n')
        || file_name.contains('\r')
    {
        return Err("peer piece file name must be a safe single file name".to_string());
    }
    Ok(())
}

fn require_safe_import_name(name: &str, label: &str) -> Result<(), String> {
    require_safe_piece_file_name(name).map_err(|_| {
        format!("peer import request {label} must be a safe single file or directory name")
    })?;
    if name.contains(':') || name.starts_with('.') {
        return Err(format!(
            "peer import request {label} must be a safe relative name"
        ));
    }
    Ok(())
}

fn receipt_file_name(assembled_output_slm: &str) -> Result<String, String> {
    require_safe_import_name(assembled_output_slm, "assembled_output_slm")?;
    let stem = assembled_output_slm
        .strip_suffix(".slm")
        .ok_or_else(|| "assembled_output_slm must end with .slm".to_string())?;
    Ok(format!("{stem}.minimodel-receipt"))
}

fn path_file_name_hint(path_text: &str, label: &str) -> Result<String, String> {
    Path::new(path_text)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .ok_or_else(|| format!("{label} path must include a file or directory name"))
}

fn temp_output_path(output_artifact_path: &Path) -> Result<std::path::PathBuf, String> {
    let file_name = output_artifact_path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "output artifact path must include a file name".to_string())?;
    Ok(output_artifact_path.with_file_name(format!("{file_name}.part")))
}

fn temp_download_dir(output_dir: &Path) -> Result<std::path::PathBuf, String> {
    let file_name = output_dir
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "peer piece output path must include a directory name".to_string())?;
    Ok(output_dir.with_file_name(format!("{file_name}.download-part")))
}

#[derive(Debug, Clone)]
struct ChunkProof {
    offset: usize,
    byte_count: usize,
    sha256: String,
}

fn verify_chunks(manifest: &Manifest, artifact_bytes: &[u8]) -> Result<(), String> {
    let mode = manifest.get("chunks.mode").unwrap_or("none");
    if mode == "none" {
        return Ok(());
    }
    if mode != "fixed-size-merkle-v0" {
        return Err(format!("unsupported chunks.mode {mode}"));
    }
    let chunk_size = parse_usize(manifest.require("chunks.size")?, "chunks.size")?;
    if chunk_size == 0 {
        return Err("chunks.size must be greater than zero".to_string());
    }
    let expected_count = parse_usize(manifest.require("chunks.count")?, "chunks.count")?;
    let chunks = chunk_proofs(artifact_bytes, chunk_size)?;
    if chunks.len() != expected_count {
        return Err(format!(
            "chunk count mismatch: manifest expected {expected_count}, artifact produced {}",
            chunks.len()
        ));
    }
    let expected_root = normalize_sha256(manifest.require("chunks.merkle_root_sha256")?)?;
    let actual_root = merkle_root_sha256(&chunks)?;
    if actual_root != expected_root {
        return Err(format!(
            "chunk Merkle root mismatch: manifest expected {expected_root}, artifact produced {actual_root}"
        ));
    }
    if let Some(expected_list_sha) = manifest.get("chunks.list.sha256") {
        let expected_list_sha = normalize_sha256(expected_list_sha)?;
        let actual_list_sha = sha256_prefixed(chunk_list_text(&chunks).as_bytes());
        if actual_list_sha != expected_list_sha {
            return Err(format!(
                "chunk list SHA-256 mismatch: manifest expected {expected_list_sha}, artifact produced {actual_list_sha}"
            ));
        }
    }
    for (index, chunk) in chunks.iter().enumerate() {
        let offset_key = format!("chunk.{index}.offset");
        let count_key = format!("chunk.{index}.byte_count");
        let sha_key = format!("chunk.{index}.sha256");
        if let Some(expected) = manifest.get(&offset_key) {
            let expected = parse_usize(expected, &offset_key)?;
            if expected != chunk.offset {
                return Err(format!(
                    "{offset_key} mismatch: manifest expected {expected}, artifact produced {}",
                    chunk.offset
                ));
            }
        }
        if let Some(expected) = manifest.get(&count_key) {
            let expected = parse_usize(expected, &count_key)?;
            if expected != chunk.byte_count {
                return Err(format!(
                    "{count_key} mismatch: manifest expected {expected}, artifact produced {}",
                    chunk.byte_count
                ));
            }
        }
        if let Some(expected) = manifest.get(&sha_key) {
            let expected = normalize_sha256(expected)?;
            if expected != chunk.sha256 {
                return Err(format!(
                    "{sha_key} mismatch: manifest expected {expected}, artifact produced {}",
                    chunk.sha256
                ));
            }
        }
    }
    Ok(())
}

fn chunk_proofs(bytes: &[u8], chunk_size: usize) -> Result<Vec<ChunkProof>, String> {
    if chunk_size == 0 {
        return Err("chunk size must be greater than zero".to_string());
    }
    let mut chunks = Vec::new();
    for (index, chunk) in bytes.chunks(chunk_size).enumerate() {
        chunks.push(ChunkProof {
            offset: index
                .checked_mul(chunk_size)
                .ok_or_else(|| "chunk offset overflow".to_string())?,
            byte_count: chunk.len(),
            sha256: sha256_prefixed(chunk),
        });
    }
    if chunks.is_empty() {
        return Err("artifact must not be empty".to_string());
    }
    Ok(chunks)
}

fn chunk_list_text(chunks: &[ChunkProof]) -> String {
    let mut text = String::new();
    for (index, chunk) in chunks.iter().enumerate() {
        text.push_str(&format!(
            "{}={}={}={}\n",
            index, chunk.offset, chunk.byte_count, chunk.sha256
        ));
    }
    text
}

fn chunk_proofs_from_manifest(manifest: &Manifest) -> Result<Vec<ChunkProof>, String> {
    require_fixed_chunk_mode(manifest)?;
    let chunk_count = parse_usize(manifest.require("chunks.count")?, "chunks.count")?;
    let mut chunks = Vec::with_capacity(chunk_count);
    for index in 0..chunk_count {
        let prefix = format!("chunk.{index}");
        let offset = parse_usize(
            manifest.require(&format!("{prefix}.offset"))?,
            &format!("{prefix}.offset"),
        )?;
        let byte_count = parse_usize(
            manifest.require(&format!("{prefix}.byte_count"))?,
            &format!("{prefix}.byte_count"),
        )?;
        let sha256 = normalize_sha256(manifest.require(&format!("{prefix}.sha256"))?)?;
        chunks.push(ChunkProof {
            offset,
            byte_count,
            sha256,
        });
    }
    let expected_merkle_root = normalize_sha256(manifest.require("chunks.merkle_root_sha256")?)?;
    let actual_merkle_root = merkle_root_sha256(&chunks)?;
    if actual_merkle_root != expected_merkle_root {
        return Err(format!(
            "manifest chunks.merkle_root_sha256 mismatch: expected {expected_merkle_root}, recomputed {actual_merkle_root}"
        ));
    }
    if let Some(expected_list_sha) = manifest.get("chunks.list.sha256") {
        let expected_list_sha = normalize_sha256(expected_list_sha)?;
        let actual_list_sha = sha256_prefixed(chunk_list_text(&chunks).as_bytes());
        if actual_list_sha != expected_list_sha {
            return Err(format!(
                "manifest chunks.list.sha256 mismatch: expected {expected_list_sha}, recomputed {actual_list_sha}"
            ));
        }
    }
    Ok(chunks)
}

fn merkle_root_sha256(chunks: &[ChunkProof]) -> Result<String, String> {
    if chunks.is_empty() {
        return Err("chunk Merkle tree requires at least one chunk".to_string());
    }
    let mut level = chunks
        .iter()
        .map(|chunk| parse_sha256_hex(&chunk.sha256))
        .collect::<Result<Vec<_>, _>>()?;
    while level.len() > 1 {
        let mut next = Vec::new();
        for pair in level.chunks(2) {
            let left = &pair[0];
            let right = pair.get(1).unwrap_or(left);
            let mut bytes = Vec::with_capacity(64);
            bytes.extend_from_slice(left);
            bytes.extend_from_slice(right);
            next.push(sha256(&bytes));
        }
        level = next;
    }
    Ok(format!("sha256:{}", bytes_to_hex(&level[0])))
}

fn parse_usize(value: &str, field: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("{field} must be a decimal integer"))
}

fn push_line(text: &mut String, key: &str, value: &str) {
    text.push_str(key);
    text.push('=');
    text.push_str(value);
    text.push('\n');
}

#[derive(Debug, Clone)]
struct HttpUrl {
    host: String,
    port: u16,
    path: String,
}

impl HttpUrl {
    fn parse(url: &str) -> Result<Self, String> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| "peer piece fetch currently requires an http:// peer URL".to_string())?;
        if rest.is_empty() || rest.contains('@') || rest.contains('?') || rest.contains('#') {
            return Err("peer URL must be a plain http://host[:port]/path URL".to_string());
        }
        let (authority, path) = match rest.split_once('/') {
            Some((authority, path)) => (authority, format!("/{path}")),
            None => (rest, "/".to_string()),
        };
        if authority.is_empty() {
            return Err("peer URL host must not be empty".to_string());
        }
        if authority.starts_with('[') || authority.matches(':').count() > 1 {
            return Err(
                "peer URL IPv6 literals are not supported in the first prototype".to_string(),
            );
        }
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() => {
                let port = port
                    .parse::<u16>()
                    .map_err(|_| "peer URL port must be a decimal TCP port".to_string())?;
                (host.to_string(), port)
            }
            Some(_) => return Err("peer URL host must not be empty".to_string()),
            None => (authority.to_string(), 80),
        };
        if host.is_empty() || host.contains('/') || host.contains('\\') {
            return Err("peer URL host is invalid".to_string());
        }
        if !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        {
            return Err("peer URL host must be plain ASCII DNS or IPv4 text".to_string());
        }
        let host = normalize_peer_url_host(&host)?;
        validate_peer_url_path(&path)?;
        Ok(Self { host, port, path })
    }

    fn join_file(&self, file_name: &str) -> Result<Self, String> {
        require_safe_piece_file_name(file_name)?;
        let mut path = self.path.clone();
        if !path.ends_with('/') {
            path.push('/');
        }
        path.push_str(file_name);
        Ok(Self {
            host: self.host.clone(),
            port: self.port,
            path,
        })
    }
}

fn validate_peer_url_path(path: &str) -> Result<(), String> {
    if !path.starts_with('/') {
        return Err("peer URL path must be absolute".to_string());
    }
    if path.contains('\\')
        || path.contains("..")
        || path.contains('\n')
        || path.contains('\r')
        || path.bytes().any(|byte| byte <= 0x20 || byte == 0x7f)
    {
        return Err("peer URL path must be a plain safe path".to_string());
    }
    Ok(())
}

impl std::fmt::Display for HttpUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.port == 80 {
            write!(formatter, "http://{}{}", self.host, self.path)
        } else {
            write!(formatter, "http://{}:{}{}", self.host, self.port, self.path)
        }
    }
}

fn normalize_peer_url_host(host: &str) -> Result<String, String> {
    let normalized = host.trim_end_matches('.').to_ascii_lowercase();
    if normalized.is_empty() || normalized.starts_with('.') || normalized.contains("..") {
        return Err("peer URL host is invalid".to_string());
    }
    Ok(normalized)
}

fn reject_project_server_peer_host(host: &str) -> Result<(), String> {
    let normalized = host.trim_end_matches('.').to_ascii_lowercase();
    let lower = normalized
        .strip_prefix("www.")
        .unwrap_or(&normalized)
        .to_string();
    let forbidden = [
        "minimodel.org",
        "tinyrustlm.com",
        "slmcomposer.com",
        "lmcomposer.com",
        "runtime.mirust.com",
        "tinyrustlm.mirust.com",
        "minimodel.mirust.com",
        "mirust.com",
        "huggingface.co",
    ];
    if forbidden.contains(&lower.as_str())
        || lower.ends_with(".minimodel.org")
        || lower.ends_with(".tinyrustlm.com")
        || lower.ends_with(".slmcomposer.com")
        || lower.ends_with(".lmcomposer.com")
        || lower.ends_with(".mirust.com")
        || lower.ends_with(".huggingface.co")
    {
        return Err(format!(
            "peer piece URL host {host} is not a peer; project servers never serve model bytes, and Hugging Face hosts never serve model bytes"
        ));
    }
    Ok(())
}

fn http_get_bytes(url: &HttpUrl, max_body_bytes: usize) -> Result<Vec<u8>, String> {
    let mut stream = TcpStream::connect((url.host.as_str(), url.port))
        .map_err(|error| format!("failed to connect to peer {}: {error}", url.host))?;
    let request = format!(
        "GET {} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\nUser-Agent: minimodel-peer-fetch/0\r\n\r\n",
        url.path, url.host
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("failed to request peer piece: {error}"))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| format!("failed to read peer response: {error}"))?;
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "peer HTTP response is missing a header terminator".to_string())?;
    let header_bytes = &response[..header_end];
    let body = response[header_end + 4..].to_vec();
    if body.len() > max_body_bytes {
        return Err(format!(
            "peer response body exceeded expected byte limit {max_body_bytes}"
        ));
    }
    let header = std::str::from_utf8(header_bytes)
        .map_err(|_| "peer HTTP response headers must be UTF-8".to_string())?;
    let mut lines = header.lines();
    let status = lines
        .next()
        .ok_or_else(|| "peer HTTP response is missing a status line".to_string())?;
    if !(status.starts_with("HTTP/1.0 200 ") || status.starts_with("HTTP/1.1 200 ")) {
        return Err(format!("peer HTTP request failed: {status}"));
    }
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("transfer-encoding")
                && value.to_ascii_lowercase().contains("chunked")
            {
                return Err("peer HTTP chunked transfer encoding is not supported".to_string());
            }
            if name.trim().eq_ignore_ascii_case("content-length") {
                let expected = value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| "peer HTTP content-length must be decimal".to_string())?;
                if expected != body.len() {
                    return Err(format!(
                        "peer HTTP content-length mismatch: header {expected}, body {}",
                        body.len()
                    ));
                }
            }
        }
    }
    Ok(body)
}

fn utc_now() -> Result<String, String> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before UNIX epoch".to_string())?
        .as_secs() as i64;
    Ok(format_unix_utc(seconds))
}

fn parse_utc_timestamp_seconds(value: &str, label: &str) -> Result<i64, String> {
    let bytes = value.as_bytes();
    let shape_ok = bytes.len() == 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z';
    if !shape_ok {
        return Err(format!("{label} must be an ISO-like UTC timestamp"));
    }

    let year = parse_timestamp_digits(bytes, 0, 4, label, "year")?;
    let month = parse_timestamp_digits(bytes, 5, 7, label, "month")?;
    let day = parse_timestamp_digits(bytes, 8, 10, label, "day")?;
    let hour = parse_timestamp_digits(bytes, 11, 13, label, "hour")?;
    let minute = parse_timestamp_digits(bytes, 14, 16, label, "minute")?;
    let second = parse_timestamp_digits(bytes, 17, 19, label, "second")?;

    if !(1..=12).contains(&month) {
        return Err(format!("{label} month must be in 01..12"));
    }
    let max_day = days_in_month(year, month);
    if day < 1 || day > max_day {
        return Err(format!("{label} day must be valid for the month"));
    }
    if hour > 23 {
        return Err(format!("{label} hour must be in 00..23"));
    }
    if minute > 59 {
        return Err(format!("{label} minute must be in 00..59"));
    }
    if second > 59 {
        return Err(format!("{label} second must be in 00..59"));
    }

    let days = days_from_civil(year, month, day);
    Ok(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn parse_timestamp_digits(
    bytes: &[u8],
    start: usize,
    end: usize,
    label: &str,
    component: &str,
) -> Result<i64, String> {
    let mut value = 0i64;
    for byte in &bytes[start..end] {
        if !byte.is_ascii_digit() {
            return Err(format!("{label} {component} must be decimal digits"));
        }
        value = value * 10 + i64::from(byte - b'0');
    }
    Ok(value)
}

fn format_unix_utc(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    const REAL_Q8_MANIFEST: &str =
        include_str!("../../../examples/tiny-test-model-q8.minimodel.manifest.v0");
    const REAL_Q8_PEER_OFFER: &str =
        include_str!("../../../examples/tiny-test-model-q8.minimodel-peer-offer");
    const REAL_Q8_PEER_IMPORT_REQUEST: &str =
        include_str!("../../../examples/tiny-test-model-q8.minimodel-peer-import-request.json");

    #[test]
    fn formats_unix_epoch_utc() {
        assert_eq!(format_unix_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_utc(1_782_950_400), "2026-07-02T00:00:00Z");
    }

    #[test]
    fn rejects_unsafe_peer_url_parts() {
        assert!(HttpUrl::parse("http://127.0.0.1:8123/peer-pieces/").is_ok());
        assert!(HttpUrl::parse("http://localhost:8123/peer-pieces/").is_ok());

        for url in [
            "http://bad host:8123/peer-pieces/",
            "http://127.0.0.1:8123/peer-pieces/../secret/",
            "http://127.0.0.1:8123/peer pieces/",
            "http://127.0.0.1:8123/peer-pieces/\r\nX-Bad: yes",
            "http://127.0.0.1:8123/peer-pieces\\secret/",
        ] {
            assert!(HttpUrl::parse(url).is_err(), "{url} should be rejected");
        }
    }

    #[test]
    fn verifies_real_q8_manifest_and_sibling_artifact_when_present() {
        let Some(path) = sibling_q8_fixture() else {
            eprintln!("skipping artifact verification test; TinyRustLM fixture not present");
            return;
        };
        let manifest = Manifest::parse(REAL_Q8_MANIFEST).unwrap();
        manifest.validate().unwrap();
        let artifact = std::fs::read(path).unwrap();
        let report =
            verify_artifact_bytes(&manifest, REAL_Q8_MANIFEST.as_bytes(), &artifact).unwrap();
        assert_eq!(report.artifact_byte_count, 8832);
        assert_eq!(
            report.artifact_sha256,
            "sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf"
        );
        assert_eq!(report.slm.quantization, "q8_0");
    }

    #[test]
    fn rejects_artifact_byte_count_drift_when_fixture_present() {
        let Some(path) = sibling_q8_fixture() else {
            eprintln!("skipping artifact drift test; TinyRustLM fixture not present");
            return;
        };
        let manifest = Manifest::parse(
            &REAL_Q8_MANIFEST.replace("artifact.byte_count=8832", "artifact.byte_count=8833"),
        )
        .unwrap();
        manifest.validate().unwrap();
        let artifact = std::fs::read(path).unwrap();
        let error =
            verify_artifact_bytes(&manifest, REAL_Q8_MANIFEST.as_bytes(), &artifact).unwrap_err();
        assert!(error.contains("byte count"));
        manifest
            .fields()
            .get("artifact.byte_count")
            .expect("manifest field used above");
    }

    #[test]
    fn rejects_artifact_sha256_drift_when_fixture_present() {
        let Some(path) = sibling_q8_fixture() else {
            eprintln!("skipping artifact SHA drift test; TinyRustLM fixture not present");
            return;
        };
        let manifest_text = REAL_Q8_MANIFEST.replace(
            "artifact.sha256=sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf",
            "artifact.sha256=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        );
        let manifest = Manifest::parse(&manifest_text).unwrap();
        manifest.validate().unwrap();
        let artifact = std::fs::read(path).unwrap();
        let error =
            verify_artifact_bytes(&manifest, manifest_text.as_bytes(), &artifact).unwrap_err();
        assert!(error.contains("SHA-256 mismatch"));
    }

    #[test]
    fn receipt_uses_basename_only() {
        let report = ArtifactVerification {
            manifest_sha256:
                "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
            manifest_model_id: "tinyrustlm/tiny-test-model-q8".to_string(),
            artifact_byte_count: 8832,
            artifact_sha256:
                "sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf"
                    .to_string(),
            evidence_status: "passed".to_string(),
            slm: SlmReport {
                file_bytes: 8832,
                format_version: 1,
                model_shape:
                    "vocab=260,hidden=8,layers=1,heads=2,kv_heads=2,head_dim=4,ffn=16,context=16"
                        .to_string(),
                quantization: "q8_0".to_string(),
                checksum: "0x481f1037d57ebcd3".to_string(),
                tokenizer_checksum: "0x288f65d94c26743e".to_string(),
                tensor_layout_checksum: "0xd8fb6e636a8badd6".to_string(),
            },
        };
        let receipt = import_receipt_text(
            &report,
            Path::new("C:/Users/AI/Downloads/tiny-test-model-q8.slm"),
        )
        .unwrap();
        assert!(receipt.contains("artifact.local_path_hint=tiny-test-model-q8.slm"));
        assert!(!receipt.contains("C:/Users/AI/Downloads"));
        assert!(receipt.contains("import.next_gate=user-selected-tinyrustlm-local-file-open"));
    }

    #[test]
    fn utc_timestamp_parser_rejects_invalid_calendar_dates() {
        assert_eq!(
            parse_utc_timestamp_seconds("1970-01-01T00:00:00Z", "timestamp").unwrap(),
            0
        );
        assert!(parse_utc_timestamp_seconds("2024-02-29T23:59:59Z", "timestamp").is_ok());
        let error = parse_utc_timestamp_seconds("2026-02-29T00:00:00Z", "timestamp").unwrap_err();
        assert!(error.contains("day must be valid for the month"));
        let error = parse_utc_timestamp_seconds("2026-01-01T24:00:00Z", "timestamp").unwrap_err();
        assert!(error.contains("hour must be in 00..23"));
    }

    #[test]
    fn exports_and_assembles_peer_pieces_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer piece round trip test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-piece-round-trip");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let piece_dir = dir.join("pieces");
        let export = export_peer_pieces_path(&manifest_path, &artifact_path, &piece_dir).unwrap();
        assert_eq!(export.piece_count, 1);
        assert_eq!(
            export.artifact_sha256,
            "sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf"
        );
        let piece_set = std::fs::read_to_string(&export.piece_set_path).unwrap();
        assert!(piece_set.contains("piece_set.kind=minimodel.peer_piece_set.v0"));
        assert!(piece_set.contains("transfer_network_available=false"));
        assert!(piece_set.contains("project_server_serves_model_bytes=false"));
        let output = dir.join("assembled-q8.slm");
        let report = assemble_peer_pieces_path(&manifest_path, &piece_dir, &output).unwrap();
        assert_eq!(report.artifact_sha256, export.artifact_sha256);
        assert_eq!(
            std::fs::read(output).unwrap(),
            std::fs::read(artifact_path).unwrap()
        );
    }

    #[test]
    fn rejects_peer_piece_payload_drift_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer piece drift test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-piece-drift");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let piece_dir = dir.join("pieces");
        export_peer_pieces_path(&manifest_path, &artifact_path, &piece_dir).unwrap();
        let piece_path = piece_dir.join("piece-00000000.bin");
        let mut piece = std::fs::read(&piece_path).unwrap();
        piece[0] ^= 0x01;
        std::fs::write(&piece_path, piece).unwrap();
        let output = dir.join("assembled-drift.slm");
        let error = assemble_peer_pieces_path(&manifest_path, &piece_dir, &output).unwrap_err();
        assert!(error.contains("piece.0.sha256"), "{error}");
        assert!(!output.exists());
    }

    #[test]
    fn inspect_peer_piece_dir_rejects_payload_drift_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer piece serve drift test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-piece-serve-drift");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let piece_dir = dir.join("pieces");
        export_peer_pieces_path(&manifest_path, &artifact_path, &piece_dir).unwrap();
        let piece_path = piece_dir.join("piece-00000000.bin");
        let mut piece = std::fs::read(&piece_path).unwrap();
        piece[0] ^= 0x01;
        std::fs::write(&piece_path, piece).unwrap();
        let error = inspect_peer_piece_dir_path(&manifest_path, &piece_dir).unwrap_err();
        assert!(error.contains("piece.0.sha256"));
    }

    #[test]
    fn fetches_peer_pieces_over_local_http_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping HTTP peer fetch test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-piece-http-fetch");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let served_piece_dir = dir.join("served-pieces");
        export_peer_pieces_path(&manifest_path, &artifact_path, &served_piece_dir).unwrap();
        let peer_url = start_peer_piece_server(served_piece_dir, 3);
        let fetched_piece_dir = dir.join("fetched-pieces");
        let fetch = fetch_peer_pieces_http_path(&manifest_path, &peer_url, &fetched_piece_dir)
            .expect("HTTP peer fetch should pass");
        assert_eq!(fetch.piece_count, 1);
        assert_eq!(fetch.companion_state_status, "not-advertised");
        assert!(std::path::Path::new(&fetch.piece_set_path).exists());
        assert_eq!(
            fetch.artifact_sha256,
            "sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf"
        );
        let output = dir.join("fetched-assembled-q8.slm");
        let report = assemble_peer_pieces_path(&manifest_path, &fetched_piece_dir, &output)
            .expect("fetched pieces should assemble");
        assert_eq!(report.artifact_sha256, fetch.artifact_sha256);
    }

    #[test]
    fn executes_peer_import_request_over_local_http_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer import request test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-import-request");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let served_piece_dir = dir.join("served-pieces");
        export_peer_pieces_path(&manifest_path, &artifact_path, &served_piece_dir).unwrap();
        let peer_url = start_peer_piece_server(served_piece_dir, 3);
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        std::fs::write(&request_path, peer_import_request_json(&peer_url)).unwrap();
        let output_dir = dir.join("imported");

        let report =
            execute_peer_import_request_path(&request_path, &manifest_path, &output_dir).unwrap();

        assert_eq!(report.peer_base_url, peer_url);
        assert_eq!(report.companion_state_status, "not-advertised");
        assert_eq!(report.piece_count, 1);
        assert!(Path::new(&report.piece_dir_path).exists());
        assert!(Path::new(&report.output_artifact_path).exists());
        assert!(Path::new(&report.receipt_path).exists());
        assert_eq!(
            std::fs::read(&report.output_artifact_path).unwrap(),
            std::fs::read(artifact_path).unwrap()
        );
        let receipt = std::fs::read_to_string(&report.receipt_path).unwrap();
        assert!(receipt.contains("slm.validation_status=passed"));
        assert!(receipt.contains("artifact.local_path_hint=tiny-test-model-q8.slm"));
        assert!(receipt.contains("peer_import.transport=known-direct-http-peer-v0"));
        assert!(receipt.contains("peer_import.verification_status=passed"));
        assert!(receipt.contains("peer.base_url="));
        assert!(receipt.contains("peer.piece_dir_path_hint=tiny-test-model-q8-peer-pieces"));
        assert!(receipt.contains("peer.piece_set_path_hint=minimodel.peer_piece_set.v0"));
        assert!(receipt.contains("peer.piece_set_sha256=sha256:"));
        assert!(receipt.contains("peer.companion_state_status=not-advertised"));
        assert!(receipt.contains("peer_companion_state_metadata_fetched=false"));
        assert!(receipt.contains("peer_piece_bytes_fetched=true"));
        assert!(receipt.contains("artifact.assembled_from_peer_pieces=true"));
        assert!(receipt.contains("project_server_serves_model_bytes=false"));
    }

    #[test]
    fn executes_peer_import_request_validates_companion_state_when_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping companion-aware peer import test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-import-request-companion");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let served_piece_dir = dir.join("served-pieces");
        export_peer_pieces_path(&manifest_path, &artifact_path, &served_piece_dir).unwrap();
        let state_path = dir.join("tiny-test-model-q8.peer-companion-state.v0.json");
        let peer_url = start_peer_companion_server_for_share(
            manifest_path.clone(),
            served_piece_dir,
            state_path,
            6,
        );
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        std::fs::write(&request_path, peer_import_request_json(&peer_url)).unwrap();
        let preflight_path = dir.join("tiny-test-model-q8.peer-import.preflight");
        let output_dir = dir.join("imported");

        let preflight =
            preflight_peer_import_request_path(&request_path, &manifest_path, &preflight_path)
                .unwrap();
        let report =
            execute_peer_import_request_path(&request_path, &manifest_path, &output_dir).unwrap();

        assert_eq!(report.peer_base_url, peer_url);
        assert_eq!(report.companion_state_status, "validated");
        assert!(report.piece_set_sha256.starts_with("sha256:"));
        assert!(report
            .companion_state_url
            .ends_with(PEER_COMPANION_STATE_ROUTE));
        assert_eq!(report.piece_count, 1);
        assert_eq!(
            std::fs::read(&report.output_artifact_path).unwrap(),
            std::fs::read(artifact_path).unwrap()
        );
        let receipt = std::fs::read_to_string(&report.receipt_path).unwrap();
        let preflight_text = std::fs::read_to_string(&preflight_path).unwrap();
        assert!(receipt.contains("peer.companion_state_status=validated"));
        assert!(receipt.contains("peer_companion_state_metadata_fetched=true"));
        assert!(receipt.contains("peer_piece_bytes_fetched=true"));
        assert!(receipt.contains("chunks.merkle_root_sha256=sha256:"));
        let chain = inspect_peer_import_receipt_path(
            Path::new(&report.receipt_path),
            &request_path,
            &preflight_path,
            &manifest_path,
            Path::new(&report.output_artifact_path),
        )
        .unwrap();
        assert_eq!(chain.peer_base_url, peer_url);
        assert_eq!(
            chain.preflight_sha256,
            sha256_prefixed(&std::fs::read(&preflight_path).unwrap())
        );
        assert_eq!(chain.piece_set_sha256, preflight.piece_set_sha256);
        assert!(chain.peer_companion_state_metadata_fetched);
        assert!(chain.peer_piece_bytes_fetched);
        assert!(chain.artifact_assembled_from_peer_pieces);
        assert!(!chain.project_server_serves_model_bytes);

        let drift_receipt_path =
            output_dir.join("tiny-test-model-q8.piece-set-drift.minimodel-receipt");
        std::fs::write(
            &drift_receipt_path,
            receipt.replace(
                &format!("peer.piece_set_sha256={}", preflight.piece_set_sha256),
                "peer.piece_set_sha256=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
        )
        .unwrap();
        let error = inspect_peer_import_receipt_path(
            &drift_receipt_path,
            &request_path,
            &preflight_path,
            &manifest_path,
            Path::new(&report.output_artifact_path),
        )
        .unwrap_err();
        assert!(error.contains("peer.piece_set_sha256"), "{error}");

        let future_preflight_path =
            output_dir.join("tiny-test-model-q8.future.peer-import.preflight");
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
        .unwrap();
        let error = inspect_peer_import_receipt_path(
            Path::new(&report.receipt_path),
            &request_path,
            &future_preflight_path,
            &manifest_path,
            Path::new(&report.output_artifact_path),
        )
        .unwrap_err();
        assert!(error.contains("receipt.created_utc"), "{error}");
        assert!(error.contains("preflight.created_utc"), "{error}");
    }

    #[test]
    fn executes_peer_import_request_through_companion_subpath_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!(
                "skipping companion subpath peer import test; TinyRustLM fixture not present"
            );
            return;
        };
        let dir = unique_dir("peer-import-request-companion-subpath");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let served_piece_dir = dir.join("served-pieces");
        export_peer_pieces_path(&manifest_path, &artifact_path, &served_piece_dir).unwrap();
        let state_path = dir.join("tiny-test-model-q8.peer-companion-state.v0.json");
        let peer_url = start_peer_companion_server_for_share_with_path(
            manifest_path.clone(),
            served_piece_dir,
            state_path,
            "peer-pieces",
            3,
        );
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        std::fs::write(&request_path, peer_import_request_json(&peer_url)).unwrap();
        let output_dir = dir.join("imported");

        let report =
            execute_peer_import_request_path(&request_path, &manifest_path, &output_dir).unwrap();

        assert!(peer_url.ends_with("/peer-pieces/"));
        assert_eq!(report.peer_base_url, peer_url);
        assert_eq!(report.companion_state_status, "validated");
        assert!(report
            .companion_state_url
            .ends_with("/peer-pieces/minimodel.peer_companion_state.v0.json"));
        assert_eq!(
            std::fs::read(&report.output_artifact_path).unwrap(),
            std::fs::read(artifact_path).unwrap()
        );
        let receipt = std::fs::read_to_string(&report.receipt_path).unwrap();
        assert!(receipt.contains("peer.base_url="));
        assert!(receipt.contains("/peer-pieces/"));
        assert!(receipt.contains("peer_piece_bytes_fetched=true"));
    }

    #[test]
    fn preflights_peer_import_request_metadata_only_over_local_http_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer import preflight test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-import-preflight");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let served_piece_dir = dir.join("served-pieces");
        export_peer_pieces_path(&manifest_path, &artifact_path, &served_piece_dir).unwrap();
        let peer_url = start_peer_piece_server(served_piece_dir, 3);
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        let mut request_value: Value =
            serde_json::from_str(&peer_import_request_json(&peer_url)).unwrap();
        let piece_contract = request_value
            .get_mut("local_peer_piece_contract")
            .and_then(Value::as_object_mut)
            .unwrap();
        piece_contract.remove("metadata_routes");
        piece_contract.remove("receiver_validation_contract");
        std::fs::write(
            &request_path,
            serde_json::to_string_pretty(&request_value).unwrap(),
        )
        .unwrap();
        let preflight_path = dir.join("tiny-test-model-q8.peer-import.preflight");

        let report =
            preflight_peer_import_request_path(&request_path, &manifest_path, &preflight_path)
                .unwrap();

        assert_eq!(report.peer_base_url, peer_url);
        assert_eq!(report.companion_state_status, "not-advertised");
        assert_eq!(report.metadata_routes_status, "not-declared");
        assert_eq!(report.peer_status_route, PEER_STATUS_ROUTE);
        assert_eq!(report.peer_piece_set_route, PEER_PIECE_SET_ROUTE);
        assert_eq!(
            report.peer_companion_state_route,
            PEER_COMPANION_STATE_ROUTE
        );
        assert_eq!(report.receiver_validation_contract_status, "not-declared");
        assert!(!report.check_peer_status_before_piece_fetch);
        assert!(!report.preflight_fetches_piece_set_before_pieces);
        assert!(!report.validate_advertised_companion_state_before_piece_fetch);
        assert!(!report.import_receipt_required_before_browser_load);
        assert_eq!(report.piece_count, 1);
        assert!(preflight_path.exists());
        let preflight = std::fs::read_to_string(&preflight_path).unwrap();
        assert!(preflight.contains("preflight.kind=minimodel.peer_import_preflight"));
        assert!(preflight.contains("metadata_routes.status=not-declared"));
        assert!(preflight.contains("metadata_routes.peer_status=minimodel.peer_status.v0.json"));
        assert!(preflight.contains("metadata_routes.peer_piece_set=minimodel.peer_piece_set.v0"));
        assert!(preflight.contains(
            "metadata_routes.peer_companion_state=minimodel.peer_companion_state.v0.json"
        ));
        assert!(preflight.contains("receiver_validation_contract.status=not-declared"));
        assert!(
            preflight.contains("receiver_validation.check_peer_status_before_piece_fetch=false")
        );
        assert!(preflight
            .contains("receiver_validation.preflight_fetches_piece_set_before_pieces=false"));
        assert!(preflight.contains(
            "receiver_validation.validate_advertised_companion_state_before_piece_fetch=false"
        ));
        assert!(preflight
            .contains("receiver_validation.import_receipt_required_before_browser_load=false"));
        assert!(preflight.contains("piece_set.validation_status=passed"));
        assert!(preflight.contains("piece_set.sha256=sha256:"));
        assert!(preflight.contains("peer_piece_set_metadata_fetched=true"));
        assert!(preflight.contains("peer.companion_state_status=not-advertised"));
        assert!(preflight.contains("peer_companion_state_metadata_fetched=false"));
        assert!(preflight.contains("peer_piece_bytes_fetched=false"));
        assert!(preflight.contains("artifact_bytes_fetched=false"));
        assert!(preflight.contains("project_server_serves_model_bytes=false"));
        assert!(preflight.contains(
            "required_next_gate=run-import-peer-request-fetch-pieces-assemble-rust-reverify"
        ));
    }

    #[test]
    fn preflights_peer_import_request_validates_companion_state_when_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping companion-aware peer import preflight test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-import-preflight-companion");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let served_piece_dir = dir.join("served-pieces");
        export_peer_pieces_path(&manifest_path, &artifact_path, &served_piece_dir).unwrap();
        let state_path = dir.join("tiny-test-model-q8.peer-companion-state.v0.json");
        let peer_url = start_peer_companion_server_for_share(
            manifest_path.clone(),
            served_piece_dir,
            state_path,
            2,
        );
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        std::fs::write(&request_path, peer_import_request_json(&peer_url)).unwrap();
        let preflight_path = dir.join("tiny-test-model-q8.peer-import.preflight");

        let report =
            preflight_peer_import_request_path(&request_path, &manifest_path, &preflight_path)
                .unwrap();

        assert_eq!(report.peer_base_url, peer_url);
        assert_eq!(report.companion_state_status, "validated");
        assert_eq!(report.metadata_routes_status, "validated");
        assert_eq!(report.peer_status_route, PEER_STATUS_ROUTE);
        assert_eq!(report.peer_piece_set_route, PEER_PIECE_SET_ROUTE);
        assert_eq!(
            report.peer_companion_state_route,
            PEER_COMPANION_STATE_ROUTE
        );
        assert_eq!(report.receiver_validation_contract_status, "validated");
        assert!(report.check_peer_status_before_piece_fetch);
        assert!(report.preflight_fetches_piece_set_before_pieces);
        assert!(report.validate_advertised_companion_state_before_piece_fetch);
        assert!(report.import_receipt_required_before_browser_load);
        let preflight = std::fs::read_to_string(&preflight_path).unwrap();
        assert!(preflight.contains("peer.companion_state_status=validated"));
        assert!(preflight.contains("metadata_routes.status=validated"));
        assert!(preflight.contains("metadata_routes.peer_status=minimodel.peer_status.v0.json"));
        assert!(preflight.contains("metadata_routes.peer_piece_set=minimodel.peer_piece_set.v0"));
        assert!(preflight.contains(
            "metadata_routes.peer_companion_state=minimodel.peer_companion_state.v0.json"
        ));
        assert!(preflight.contains("receiver_validation_contract.status=validated"));
        assert!(preflight.contains("receiver_validation.check_peer_status_before_piece_fetch=true"));
        assert!(preflight
            .contains("receiver_validation.preflight_fetches_piece_set_before_pieces=true"));
        assert!(preflight.contains(
            "receiver_validation.validate_advertised_companion_state_before_piece_fetch=true"
        ));
        assert!(preflight
            .contains("receiver_validation.import_receipt_required_before_browser_load=true"));
        assert!(preflight.contains("peer_companion_state_metadata_fetched=true"));
        assert!(preflight.contains("peer_piece_bytes_fetched=false"));
        assert!(preflight.contains("artifact_bytes_fetched=false"));

        let inspection =
            inspect_peer_import_preflight_path(&preflight_path, &request_path, &manifest_path)
                .unwrap();
        assert_eq!(inspection.peer_base_url, peer_url);
        assert_eq!(inspection.companion_state_status, "validated");
        assert_eq!(inspection.metadata_routes_status, "validated");
        assert_eq!(inspection.receiver_validation_contract_status, "validated");
        assert!(inspection.peer_piece_set_metadata_fetched);
        assert!(inspection.peer_companion_state_metadata_fetched);
        assert!(!inspection.peer_piece_bytes_fetched);
        assert!(!inspection.artifact_bytes_fetched);

        let drift_path = dir.join("tiny-test-model-q8.drift.peer-import.preflight");
        std::fs::write(
            &drift_path,
            preflight.replace(
                "artifact_bytes_fetched=false",
                "artifact_bytes_fetched=true",
            ),
        )
        .unwrap();
        let error = inspect_peer_import_preflight_path(&drift_path, &request_path, &manifest_path)
            .unwrap_err();
        assert!(error.contains("artifact_bytes_fetched mismatch"));
    }

    #[test]
    fn preflight_rejects_peer_companion_state_drift_when_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping companion drift preflight test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-import-preflight-companion-drift");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let served_piece_dir = dir.join("served-pieces");
        export_peer_pieces_path(&manifest_path, &artifact_path, &served_piece_dir).unwrap();
        let state_path = dir.join("tiny-test-model-q8.peer-companion-state.v0.json");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let peer_url = format!("http://{address}/");
        write_peer_companion_state_path(&manifest_path, &served_piece_dir, &state_path, &peer_url)
            .unwrap();
        let drifted_state = std::fs::read_to_string(&state_path)
            .unwrap()
            .replace("\"piece_count\": 1", "\"piece_count\": 2");
        std::fs::write(&state_path, drifted_state).unwrap();
        let mut extra_files = BTreeMap::new();
        extra_files.insert(
            PEER_COMPANION_STATE_ROUTE.to_string(),
            ServedPeerExtra::Path(state_path),
        );
        std::thread::spawn(move || {
            serve_peer_piece_dir_http_listener_with_extra(
                &served_piece_dir,
                listener,
                Some(2),
                extra_files,
                "/",
            )
            .unwrap();
        });
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        std::fs::write(&request_path, peer_import_request_json(&peer_url)).unwrap();
        let preflight_path = dir.join("tiny-test-model-q8.peer-import.preflight");

        let error =
            preflight_peer_import_request_path(&request_path, &manifest_path, &preflight_path)
                .unwrap_err();

        assert!(error.contains("peer companion state pieces.piece_count mismatch"));
        assert!(!preflight_path.exists());
    }

    #[test]
    fn writes_peer_companion_state_for_verified_share_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer companion state test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-companion-state");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let piece_dir = dir.join("tiny-test-model-q8.peer-pieces");
        let state_path = dir.join("tiny-test-model-q8.peer-companion-state.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        export_peer_pieces_path(&manifest_path, &artifact_path, &piece_dir).unwrap();

        let report = write_peer_companion_state_path(
            &manifest_path,
            &piece_dir,
            &state_path,
            "http://127.0.0.1:8123/peer-pieces",
        )
        .unwrap();
        let text = std::fs::read_to_string(&state_path).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();

        assert_eq!(report.model_id, "tinyrustlm/tiny-test-model-q8");
        assert_eq!(report.peer_base_url, "http://127.0.0.1:8123/peer-pieces/");
        assert_eq!(report.piece_count, 1);
        assert_eq!(
            value["state_kind"].as_str(),
            Some("minimodel.peer_companion_state")
        );
        assert_eq!(value["share_status"].as_str(), Some("ready-to-serve"));
        assert_eq!(
            value["consent_status"].as_str(),
            Some("operator-reviewed-consenting-peer")
        );
        assert_eq!(
            value["manifest"]["sha256"].as_str(),
            Some(report.manifest_sha256.as_str())
        );
        assert_eq!(
            value["pieces"]["merkle_root_sha256"].as_str(),
            Some(report.merkle_root_sha256.as_str())
        );
        assert_eq!(
            value["boundary"]["peer_piece_bytes_in_state"].as_bool(),
            Some(false)
        );
        assert_eq!(
            value["boundary"]["project_server_serves_model_bytes"].as_bool(),
            Some(false)
        );
        assert!(!text.contains("artifact_project_server_url\": \"http"));
        assert!(!text.contains(&artifact_path.display().to_string()));
    }

    #[test]
    fn rejects_peer_companion_state_project_server_peer_before_output() {
        let dir = unique_dir("peer-companion-state-project-host");
        let _ = std::fs::remove_dir_all(&dir);
        let state_path = dir.join("state.json");

        let error = write_peer_companion_state_path(
            Path::new("missing-manifest"),
            Path::new("missing-pieces"),
            &state_path,
            "http://MiniModel.org/peer-pieces",
        )
        .unwrap_err();

        assert!(error.contains("project server"));
        assert!(!state_path.exists());
    }

    #[test]
    fn writes_and_inspects_peer_contact_card() {
        let dir = unique_dir("peer-contact-card");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let card_path = dir.join("known-peer.card.json");

        let write_report = write_peer_contact_card_path(
            &card_path,
            "http://127.0.0.1:8123/peer-pieces",
            Some("Local lab peer"),
        )
        .unwrap();
        let inspect_report = inspect_peer_contact_card_path(&card_path).unwrap();
        let text = std::fs::read_to_string(&card_path).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();

        assert_eq!(write_report.peer_label, "Local lab peer");
        assert_eq!(inspect_report.peer_label, "Local lab peer");
        assert_eq!(
            inspect_report.peer_base_url,
            "http://127.0.0.1:8123/peer-pieces/"
        );
        assert_eq!(
            value["card_kind"].as_str(),
            Some("minimodel.peer_contact_card")
        );
        assert_eq!(
            value["transfer_protocol"].as_str(),
            Some("direct-http-piece-server-v0")
        );
        assert_eq!(value["boundary"]["metadata_only"].as_bool(), Some(true));
        assert_eq!(
            value["project_server_serves_model_bytes"].as_bool(),
            Some(false)
        );
        assert_eq!(
            value["project_server_proxies_model_bytes"].as_bool(),
            Some(false)
        );
        assert_eq!(
            value["boundary"]["peer_contact_card_embeds_model_bytes"].as_bool(),
            Some(false)
        );
        assert!(!text.contains(".slm"));
        assert!(!text.contains("artifact_project_server_url"));
    }

    #[test]
    fn normalizes_known_peer_root_dot_and_host_case() {
        let normalized =
            normalize_known_peer_base_url("http://Peer.Example.test.:8123/peer-pieces").unwrap();

        assert_eq!(normalized, "http://peer.example.test:8123/peer-pieces/");
    }

    #[test]
    fn rejects_known_peer_project_huggingface_and_malformed_hosts() {
        let cases = [
            ("project-root-dot", "http://MiniModel.org./peer-pieces/"),
            (
                "project-subdomain",
                "http://catalog.minimodel.org:8123/peer-pieces/",
            ),
            ("huggingface-root", "http://huggingface.co/peer-pieces/"),
            ("huggingface-www", "http://www.huggingface.co/peer-pieces/"),
            (
                "huggingface-subdomain",
                "http://cdn.huggingface.co:8123/peer-pieces/",
            ),
        ];
        for (label, peer_url) in cases {
            let error = normalize_known_peer_base_url(peer_url).unwrap_err();
            assert!(error.contains("not a peer"), "{label}: {error}");
            assert!(
                error.contains("never serve model bytes"),
                "{label}: {error}"
            );
        }

        for peer_url in [
            "http://.example.com/peer-pieces/",
            "http://example..com/peer-pieces/",
        ] {
            let error = normalize_known_peer_base_url(peer_url).unwrap_err();
            assert!(error.contains("host is invalid"), "{peer_url}: {error}");
        }
    }

    #[test]
    fn rejects_peer_contact_card_project_server_before_output() {
        let dir = unique_dir("peer-contact-card-project-host");
        let _ = std::fs::remove_dir_all(&dir);
        let card_path = dir.join("known-peer.card.json");

        let error = write_peer_contact_card_path(
            &card_path,
            "http://MiniModel.org/peer-pieces",
            Some("bad peer"),
        )
        .unwrap_err();

        assert!(error.contains("project server"));
        assert!(!card_path.exists());
    }

    #[test]
    fn rejects_peer_contact_card_project_and_huggingface_hosts_before_output() {
        let dir = unique_dir("peer-contact-card-blocked-hosts");
        let _ = std::fs::remove_dir_all(&dir);

        let cases = [
            ("root-dot", "http://MiniModel.org./peer-pieces/"),
            ("subdomain", "http://catalog.minimodel.org/peer-pieces/"),
            ("huggingface", "http://huggingface.co/peer-pieces/"),
            (
                "huggingface-subdomain",
                "http://cdn.huggingface.co/peer-pieces/",
            ),
        ];
        for (label, peer_url) in cases {
            let card_path = dir.join(format!("{label}.card.json"));
            let error =
                write_peer_contact_card_path(&card_path, peer_url, Some("bad peer")).unwrap_err();

            assert!(error.contains("project server"), "{label}: {error}");
            assert!(
                error.contains("never serve model bytes"),
                "{label}: {error}"
            );
            assert!(!card_path.exists(), "{label} unexpectedly wrote a card");
        }
    }

    #[test]
    fn rejects_peer_contact_card_boundary_drift() {
        let dir = unique_dir("peer-contact-card-boundary-drift");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let card_path = dir.join("known-peer.card.json");
        write_peer_contact_card_path(&card_path, "http://127.0.0.1:8123/", None).unwrap();
        let mut value: Value =
            serde_json::from_str(&std::fs::read_to_string(&card_path).unwrap()).unwrap();
        value["boundary"]["project_server_serves_model_bytes"] = Value::Bool(true);
        std::fs::write(&card_path, serde_json::to_string_pretty(&value).unwrap()).unwrap();

        let error = inspect_peer_contact_card_path(&card_path).unwrap_err();

        assert!(error.contains("project_server_serves_model_bytes"));
        assert!(error.contains("must be false"));
    }

    #[test]
    fn serves_peer_status_route_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer status serve test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-status-serve");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let piece_dir = dir.join("tiny-test-model-q8.peer-pieces");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        export_peer_pieces_path(&manifest_path, &artifact_path, &piece_dir).unwrap();

        let expected = inspect_peer_piece_dir_path(&manifest_path, &piece_dir).unwrap();
        let peer_url = start_peer_status_server(manifest_path, piece_dir, 2);
        let status_url = HttpUrl::parse(&format!("{peer_url}{PEER_STATUS_ROUTE}")).unwrap();
        let status = http_get_bytes(&status_url, 32 * 1024).unwrap();
        let value: Value = serde_json::from_slice(&status).unwrap();
        assert_eq!(value["status_kind"].as_str(), Some("minimodel.peer_status"));
        assert_eq!(
            value["pieces"]["piece_set_sha256"].as_str(),
            Some(expected.piece_set_sha256.as_str())
        );
        assert_eq!(
            value["boundary"]["peer_status_embeds_model_bytes"].as_bool(),
            Some(false)
        );
        assert_eq!(
            value["boundary"]["peer_status_embeds_piece_bytes"].as_bool(),
            Some(false)
        );

        let piece_set_url =
            HttpUrl::parse(&format!("{peer_url}minimodel.peer_piece_set.v0")).unwrap();
        let piece_set = http_get_bytes(&piece_set_url, 64 * 1024).unwrap();
        let piece_set_text = std::str::from_utf8(&piece_set).unwrap();
        assert!(piece_set_text.contains("piece_set.kind=minimodel.peer_piece_set.v0"));
    }

    #[test]
    fn checks_peer_status_metadata_only_over_local_http_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer status check test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-status-check");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let piece_dir = dir.join("tiny-test-model-q8.peer-pieces");
        let status_report_path = dir.join("tiny-test-model-q8.peer-status-check.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        export_peer_pieces_path(&manifest_path, &artifact_path, &piece_dir).unwrap();
        let peer_url = start_peer_status_server(manifest_path.clone(), piece_dir, 1);

        let report =
            check_peer_status_path(&manifest_path, &peer_url, &status_report_path).unwrap();

        assert_eq!(report.peer_base_url, peer_url);
        assert_eq!(report.model_id, "tinyrustlm/tiny-test-model-q8");
        assert!(report.piece_set_sha256.starts_with("sha256:"));
        let status_report: Value =
            serde_json::from_str(&std::fs::read_to_string(&status_report_path).unwrap()).unwrap();
        assert_eq!(
            status_report["check_kind"].as_str(),
            Some("minimodel.peer_status_check")
        );
        assert_eq!(status_report["status"].as_str(), Some("validated"));
        assert_eq!(
            status_report["boundary"]["peer_piece_bytes_fetched"].as_bool(),
            Some(false)
        );
        assert_eq!(
            status_report["boundary"]["artifact_bytes_fetched"].as_bool(),
            Some(false)
        );
    }

    #[test]
    fn checks_peer_status_under_peer_url_subpath_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer status subpath check test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-status-check-subpath");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let piece_dir = dir.join("tiny-test-model-q8.peer-pieces");
        let status_report_path = dir.join("tiny-test-model-q8.peer-status-check.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        export_peer_pieces_path(&manifest_path, &artifact_path, &piece_dir).unwrap();
        let peer_url =
            start_peer_status_server_with_path(manifest_path.clone(), piece_dir, "peer-pieces", 1);

        let report =
            check_peer_status_path(&manifest_path, &peer_url, &status_report_path).unwrap();

        assert!(peer_url.ends_with("/peer-pieces/"));
        assert_eq!(report.peer_base_url, peer_url);
        assert!(report
            .status_url
            .ends_with("/peer-pieces/minimodel.peer_status.v0.json"));
        let status_report: Value =
            serde_json::from_str(&std::fs::read_to_string(&status_report_path).unwrap()).unwrap();
        assert_eq!(status_report["status"].as_str(), Some("validated"));
        assert_eq!(
            status_report["peer_base_url"].as_str(),
            Some(peer_url.as_str())
        );
    }

    #[test]
    fn rejects_peer_status_artifact_drift_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer status drift test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-status-drift");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let piece_dir = dir.join("tiny-test-model-q8.peer-pieces");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        export_peer_pieces_path(&manifest_path, &artifact_path, &piece_dir).unwrap();
        let expected = inspect_peer_piece_dir_path(&manifest_path, &piece_dir).unwrap();
        let mut status = peer_status_value(&expected, "http://127.0.0.1:8123/", None).unwrap();
        status["artifact"]["sha256"] = Value::String(
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        );

        let error =
            validate_peer_status_value(&status, &expected, "http://127.0.0.1:8123/").unwrap_err();

        assert!(error.contains("artifact.sha256 mismatch"));
    }

    #[test]
    fn serves_peer_companion_state_route_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer companion serve test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-companion-serve");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let piece_dir = dir.join("tiny-test-model-q8.peer-pieces");
        let state_path = dir.join("tiny-test-model-q8.peer-companion-state.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        export_peer_pieces_path(&manifest_path, &artifact_path, &piece_dir).unwrap();
        write_peer_companion_state_path(
            &manifest_path,
            &piece_dir,
            &state_path,
            "http://127.0.0.1:8123/",
        )
        .unwrap();
        std::fs::write(
            piece_dir.join("unlisted-companion-leak.slm"),
            b"do not serve",
        )
        .unwrap();

        let peer_url = start_peer_companion_server(state_path, manifest_path, piece_dir, 3);
        let state_url =
            HttpUrl::parse(&format!("{peer_url}minimodel.peer_companion_state.v0.json")).unwrap();
        let state = http_get_bytes(&state_url, 32 * 1024).unwrap();
        let state_text = std::str::from_utf8(&state).unwrap();
        assert!(state_text.contains("\"state_kind\": \"minimodel.peer_companion_state\""));
        assert!(state_text.contains("\"peer_piece_bytes_in_state\": false"));

        let piece_set_url =
            HttpUrl::parse(&format!("{peer_url}minimodel.peer_piece_set.v0")).unwrap();
        let piece_set = http_get_bytes(&piece_set_url, 64 * 1024).unwrap();
        let piece_set_text = std::str::from_utf8(&piece_set).unwrap();
        assert!(piece_set_text.contains("piece_set.kind=minimodel.peer_piece_set.v0"));

        let leaked_url = HttpUrl::parse(&format!("{peer_url}unlisted-companion-leak.slm")).unwrap();
        let error = http_get_bytes(&leaked_url, 1024).unwrap_err();
        assert!(error.contains("404 Not Found"));
    }

    #[test]
    fn rejects_peer_companion_state_piece_count_drift_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer companion state drift test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-companion-state-drift");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let piece_dir = dir.join("tiny-test-model-q8.peer-pieces");
        let state_path = dir.join("tiny-test-model-q8.peer-companion-state.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        export_peer_pieces_path(&manifest_path, &artifact_path, &piece_dir).unwrap();
        write_peer_companion_state_path(
            &manifest_path,
            &piece_dir,
            &state_path,
            "http://127.0.0.1:8123/",
        )
        .unwrap();
        let mut value: Value =
            serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
        value["pieces"]["piece_count"] = Value::Number(2.into());
        std::fs::write(&state_path, serde_json::to_string_pretty(&value).unwrap()).unwrap();

        let error =
            inspect_peer_companion_state_path(&state_path, &manifest_path, &piece_dir).unwrap_err();
        assert!(error.contains("pieces.piece_count mismatch"));
    }

    #[test]
    fn rejects_peer_import_request_artifact_drift_before_fetch_when_fixture_present() {
        let Some(_artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer import request drift test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-import-request-drift");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        std::fs::write(
            &request_path,
            peer_import_request_json("http://127.0.0.1:9/").replace(
                "sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf",
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
        )
        .unwrap();
        let output_dir = dir.join("imported");

        let error = execute_peer_import_request_path(&request_path, &manifest_path, &output_dir)
            .unwrap_err();

        assert!(error.contains("peer import artifact_sha256"));
        assert!(!output_dir.exists());
    }

    #[test]
    fn rejects_peer_import_request_metadata_route_drift_before_manifest_read() {
        let dir = unique_dir("peer-import-route-drift");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        let output_dir = dir.join("imported");
        let request = peer_import_request_json("http://127.0.0.1:9/").replace(
            "\"peer_companion_state\": \"minimodel.peer_companion_state.v0.json\"",
            "\"peer_companion_state\": \"../minimodel.peer_companion_state.v0.json\"",
        );
        std::fs::write(&request_path, request).unwrap();

        let error = execute_peer_import_request_path(
            &request_path,
            Path::new("missing-manifest"),
            &output_dir,
        )
        .unwrap_err();

        assert!(error.contains("peer import metadata route peer_companion_state"));
        assert!(!output_dir.exists());
    }

    #[test]
    fn rejects_peer_import_request_receiver_contract_drift_before_manifest_read() {
        let dir = unique_dir("peer-import-receiver-contract-drift");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        let output_dir = dir.join("imported");
        let request = peer_import_request_json("http://127.0.0.1:9/").replace(
            "\"validate_advertised_companion_state_before_piece_fetch\": true",
            "\"validate_advertised_companion_state_before_piece_fetch\": false",
        );
        std::fs::write(&request_path, request).unwrap();

        let error = execute_peer_import_request_path(
            &request_path,
            Path::new("missing-manifest"),
            &output_dir,
        )
        .unwrap_err();

        assert!(error.contains("validate_advertised_companion_state_before_piece_fetch"));
        assert!(!output_dir.exists());
    }

    #[test]
    fn rejects_http_peer_piece_payload_drift_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping HTTP peer drift test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-piece-http-drift");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let served_piece_dir = dir.join("served-pieces");
        export_peer_pieces_path(&manifest_path, &artifact_path, &served_piece_dir).unwrap();
        let piece_path = served_piece_dir.join("piece-00000000.bin");
        let mut piece = std::fs::read(&piece_path).unwrap();
        piece[0] ^= 0x01;
        std::fs::write(&piece_path, piece).unwrap();
        let peer_url = start_peer_piece_server(served_piece_dir, 3);
        let fetched_piece_dir = dir.join("fetched-pieces");
        let error =
            fetch_peer_pieces_http_path(&manifest_path, &peer_url, &fetched_piece_dir).unwrap_err();
        assert!(error.contains("piece.0.sha256"), "{error}");
        assert!(!fetched_piece_dir.exists());
    }

    #[test]
    fn peer_piece_server_does_not_expose_unlisted_files_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping HTTP peer exposure test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("peer-piece-http-exposure");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        let served_piece_dir = dir.join("served-pieces");
        export_peer_pieces_path(&manifest_path, &artifact_path, &served_piece_dir).unwrap();
        std::fs::write(
            served_piece_dir.join("unlisted-model-bytes.slm"),
            b"do not serve",
        )
        .unwrap();
        let peer_url = start_peer_piece_server(served_piece_dir, 1);
        let url = HttpUrl::parse(&format!("{peer_url}unlisted-model-bytes.slm")).unwrap();
        let error = http_get_bytes(&url, 1024).unwrap_err();
        assert!(error.contains("404 Not Found"));
    }

    #[test]
    fn rejects_project_server_peer_import_host_before_manifest_read() {
        let dir = unique_dir("peer-import-project-host");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
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
            let output_dir = dir.join(format!("imported-{label}"));
            std::fs::write(&request_path, peer_import_request_json(peer_url)).unwrap();

            let error = execute_peer_import_request_path(
                &request_path,
                Path::new("missing-manifest"),
                &output_dir,
            )
            .unwrap_err();

            assert!(error.contains("project server"), "{label}: {error}");
            assert!(
                error.contains("never serve model bytes"),
                "{label}: {error}"
            );
            assert!(!output_dir.exists(), "{label} unexpectedly wrote output");
        }
    }

    #[test]
    fn rejects_project_server_peer_import_preflight_host_before_manifest_read() {
        let dir = unique_dir("peer-import-preflight-project-host");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        let preflight_path = dir.join("tiny-test-model-q8.peer-import.preflight");
        std::fs::write(
            &request_path,
            peer_import_request_json("http://MiniModel.org/peer-pieces/"),
        )
        .unwrap();
        let error = preflight_peer_import_request_path(
            &request_path,
            Path::new("missing-manifest"),
            &preflight_path,
        )
        .unwrap_err();
        assert!(error.contains("project server"));
        assert!(error.contains("never serve model bytes"));
        assert!(!preflight_path.exists());
    }

    #[test]
    fn rejects_peer_import_request_missing_nested_model_field_before_manifest_read() {
        let dir = unique_dir("peer-import-missing-field");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        std::fs::write(
            &request_path,
            peer_import_request_json("http://127.0.0.1:8123/").replace(
                "    \"artifact_sha256\": \"sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf\",\n",
                "",
            ),
        )
        .unwrap();
        let error = execute_peer_import_request_path(
            &request_path,
            Path::new("missing-manifest"),
            &dir.join("imported"),
        )
        .unwrap_err();
        assert!(error.contains("artifact_sha256"));
        assert!(!dir.join("imported").exists());
    }

    #[test]
    fn rejects_peer_import_request_wrong_piece_set_kind_before_manifest_read() {
        let dir = unique_dir("peer-import-piece-set-kind");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        std::fs::write(
            &request_path,
            peer_import_request_json("http://127.0.0.1:8123/").replace(
                "minimodel.peer_piece_set.v0",
                "minimodel.unsupported_piece_set.v0",
            ),
        )
        .unwrap();
        let error = execute_peer_import_request_path(
            &request_path,
            Path::new("missing-manifest"),
            &dir.join("imported"),
        )
        .unwrap_err();
        assert!(error.contains("piece set kind"));
        assert!(!dir.join("imported").exists());
    }

    #[test]
    fn writes_catalog_entry_from_manifest_and_peer_offer() {
        let dir = unique_dir("catalog-entry");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();

        let report = write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_path,
            Some("example-org/example-source"),
            Some("http://127.0.0.1:8123/peer-pieces"),
        )
        .unwrap();
        let text = std::fs::read_to_string(&catalog_path).unwrap();

        assert_eq!(report.model_id, "tinyrustlm/tiny-test-model-q8");
        assert_eq!(
            report.artifact_sha256,
            "sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf"
        );
        assert!(text.contains("\"entry_kind\": \"minimodel.catalog_entry\""));
        assert!(text.contains("\"peer_hint_status\": \"peer-offer-ready-known-peer-needed\""));
        assert!(text.contains("\"peer_import_status\": \"request-script-ready\""));
        assert!(text.contains("\"source_huggingface_model_id\": \"example-org/example-source\""));
        assert!(text.contains("\"peer_source_count\": 1"));
        assert!(text.contains("\"peer_sources\": [{"));
        assert!(text.contains("\"peer_base_url\": \"http://127.0.0.1:8123/peer-pieces/\""));
        assert!(text.contains("\"consent_status\": \"publisher-declared-consenting-peer\""));
        assert!(text.contains("\"project_server_serves_model_bytes\": false"));
        assert!(!text.contains("artifact_project_server_url\": \"http"));
    }

    #[test]
    fn adds_peer_source_to_existing_catalog_entry() {
        let dir = unique_dir("catalog-entry-add-peer");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let catalog_entry_with_peer_path =
            dir.join("tiny-test-model-q8.catalog-entry.with-peer.v0.json");
        let catalog_snapshot_path = dir.join("models.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            Some("example-org/example-source"),
            None,
        )
        .unwrap();
        let proof_ready_text = std::fs::read_to_string(&catalog_entry_path).unwrap();
        assert!(proof_ready_text.contains("\"peer_source_count\": 0"));
        assert!(!proof_ready_text.contains("\"peer_sources\""));

        let report = add_peer_source_to_catalog_entry_path(
            &catalog_entry_path,
            &catalog_entry_with_peer_path,
            "http://127.0.0.1:8123/peer-pieces",
        )
        .unwrap();
        let text = std::fs::read_to_string(&catalog_entry_with_peer_path).unwrap();

        assert_eq!(report.model_id, "tinyrustlm/tiny-test-model-q8");
        assert_eq!(report.peer_source_count, 1);
        assert_eq!(report.peer_base_url, "http://127.0.0.1:8123/peer-pieces/");
        assert!(text.contains("\"peer_source_count\": 1"));
        assert!(text.contains("\"peer_sources\": ["));
        assert!(text.contains("\"peer_base_url\": \"http://127.0.0.1:8123/peer-pieces/\""));
        assert!(text.contains("\"project_server_serves_model_bytes\": false"));
        assert!(!text.contains("artifact_project_server_url\": \"http"));

        let snapshot_report =
            write_catalog_snapshot_path(&catalog_snapshot_path, &[catalog_entry_with_peer_path])
                .unwrap();
        assert_eq!(snapshot_report.p2p_ready_count, 1);
        assert_eq!(snapshot_report.peer_source_count, 1);
        let original_text = std::fs::read_to_string(&catalog_entry_path).unwrap();
        assert!(!original_text.contains("http://127.0.0.1:8123/peer-pieces/"));
    }

    #[test]
    fn rejects_add_peer_source_project_server_peer_before_output() {
        let dir = unique_dir("catalog-entry-add-project-peer");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let output_path = dir.join("tiny-test-model-q8.catalog-entry.with-peer.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            None,
        )
        .unwrap();

        let error = add_peer_source_to_catalog_entry_path(
            &catalog_entry_path,
            &output_path,
            "http://MiniModel.org/peer-pieces/",
        )
        .unwrap_err();

        assert!(error.contains("project server"));
        assert!(error.contains("never serve model bytes"));
        assert!(!output_path.exists());
    }

    #[test]
    fn rejects_add_peer_source_when_entry_already_has_peer_source() {
        let dir = unique_dir("catalog-entry-add-existing-peer");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let output_path = dir.join("tiny-test-model-q8.catalog-entry.replaced-peer.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            Some("http://127.0.0.1:8123/peer-pieces"),
        )
        .unwrap();

        let error = add_peer_source_to_catalog_entry_path(
            &catalog_entry_path,
            &output_path,
            "http://127.0.0.1:9123/peer-pieces",
        )
        .unwrap_err();

        assert!(error.contains("already has 1 peer source"));
        assert!(error.contains("proof-ready catalog entries"));
        assert!(!output_path.exists());
    }

    #[test]
    fn writes_and_applies_peer_source_announcement() {
        let dir = unique_dir("peer-source-announcement");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let announcement_path = dir.join("tiny-test-model-q8.peer-source-announcement.v0.json");
        let merged_entry_path = dir.join("tiny-test-model-q8.catalog-entry.announced-peer.v0.json");
        let catalog_snapshot_path = dir.join("models.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            Some("example-org/example-source"),
            None,
        )
        .unwrap();

        let announcement_report = write_peer_source_announcement_path(
            &catalog_entry_path,
            &announcement_path,
            "http://127.0.0.1:8123/peer-pieces",
        )
        .unwrap();
        assert_eq!(
            announcement_report.model_id,
            "tinyrustlm/tiny-test-model-q8"
        );
        assert_eq!(
            announcement_report.peer_base_url,
            "http://127.0.0.1:8123/peer-pieces/"
        );
        let announcement_text = std::fs::read_to_string(&announcement_path).unwrap();
        assert!(announcement_text
            .contains("\"announcement_kind\": \"minimodel.peer_source_announcement\""));
        assert!(announcement_text.contains("\"project_server_serves_model_bytes\": false"));

        let merge_report = apply_peer_source_announcements_path(
            &catalog_entry_path,
            &merged_entry_path,
            std::slice::from_ref(&announcement_path),
        )
        .unwrap();
        assert_eq!(merge_report.model_id, "tinyrustlm/tiny-test-model-q8");
        assert_eq!(merge_report.applied_announcement_count, 1);
        assert_eq!(merge_report.peer_source_count, 1);
        let merged_text = std::fs::read_to_string(&merged_entry_path).unwrap();
        assert!(merged_text.contains("\"peer_source_count\": 1"));
        assert!(merged_text.contains("\"peer_sources\": ["));
        assert!(merged_text.contains("\"peer_base_url\": \"http://127.0.0.1:8123/peer-pieces/\""));
        assert!(!merged_text.contains("artifact_project_server_url\": \"http"));

        let snapshot_report =
            write_catalog_snapshot_path(&catalog_snapshot_path, &[merged_entry_path]).unwrap();
        assert_eq!(snapshot_report.p2p_ready_count, 1);
        assert_eq!(snapshot_report.peer_source_count, 1);
    }

    #[test]
    fn writes_catalog_snapshot_with_peer_source_announcements() {
        let dir = unique_dir("peer-source-announcement-snapshot");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let announcement_a_path = dir.join("tiny-test-model-q8.peer-source-announcement-a.v0.json");
        let announcement_b_path = dir.join("tiny-test-model-q8.peer-source-announcement-b.v0.json");
        let catalog_snapshot_path = dir.join("models.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            Some("example-org/example-source"),
            None,
        )
        .unwrap();
        write_peer_source_announcement_path(
            &catalog_entry_path,
            &announcement_a_path,
            "http://127.0.0.1:8123/peer-pieces",
        )
        .unwrap();
        write_peer_source_announcement_path(
            &catalog_entry_path,
            &announcement_b_path,
            "http://127.0.0.1:9123/peer-pieces",
        )
        .unwrap();

        let report = write_catalog_snapshot_with_announcements_path(
            &catalog_snapshot_path,
            std::slice::from_ref(&catalog_entry_path),
            &[announcement_a_path, announcement_b_path],
        )
        .unwrap();
        let text = std::fs::read_to_string(&catalog_snapshot_path).unwrap();

        assert_eq!(report.entry_count, 1);
        assert_eq!(report.announcement_count, 2);
        assert_eq!(report.applied_announcement_count, 2);
        assert_eq!(report.p2p_ready_count, 1);
        assert_eq!(report.peer_source_count, 2);
        assert!(text.contains("\"catalog_kind\": \"minimodel.model_catalog\""));
        assert!(text.contains("\"peer_source_count\": 2"));
        assert!(text.contains("\"peer_base_url\": \"http://127.0.0.1:8123/peer-pieces/\""));
        assert!(text.contains("\"peer_base_url\": \"http://127.0.0.1:9123/peer-pieces/\""));
        assert!(!text.contains("artifact_project_server_url\": \"http"));
    }

    #[test]
    fn writes_peer_source_announcement_feed_and_catalog_route() {
        let dir = unique_dir("peer-source-announcement-feed");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let announcement_path = dir.join("tiny-test-model-q8.peer-source-announcement.v0.json");
        let feed_path = dir.join("peer-source-announcements.v0.json");
        let catalog_snapshot_path = dir.join("models.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            Some("example-org/example-source"),
            None,
        )
        .unwrap();
        write_peer_source_announcement_path(
            &catalog_entry_path,
            &announcement_path,
            "http://127.0.0.1:8123/peer-pieces",
        )
        .unwrap();

        let feed_report = write_peer_source_announcement_feed_path(
            &feed_path,
            std::slice::from_ref(&announcement_path),
        )
        .unwrap();
        let feed_text = std::fs::read_to_string(&feed_path).unwrap();
        assert_eq!(feed_report.announcement_count, 1);
        assert_eq!(feed_report.model_count, 1);
        assert_eq!(
            feed_report.feed_sha256,
            sha256_prefixed(feed_text.as_bytes())
        );
        assert!(feed_text.contains(
            "\"announcement_catalog_kind\": \"minimodel.peer_source_announcement_catalog\""
        ));
        assert!(feed_text.contains("\"project_server_serves_model_bytes\": false"));
        assert!(feed_text.contains("\"project_server_proxies_model_bytes\": false"));

        let snapshot_report = write_catalog_snapshot_with_announcement_feed_path(
            &catalog_snapshot_path,
            "peer-source-announcements.v0.json",
            &feed_path,
            std::slice::from_ref(&catalog_entry_path),
        )
        .unwrap();
        let snapshot_text = std::fs::read_to_string(&catalog_snapshot_path).unwrap();

        assert_eq!(snapshot_report.entry_count, 1);
        assert_eq!(snapshot_report.announcement_count, 1);
        assert_eq!(snapshot_report.announcement_model_count, 1);
        assert_eq!(
            snapshot_report.announcement_feed_route,
            "peer-source-announcements.v0.json"
        );
        assert_eq!(
            snapshot_report.announcement_feed_sha256,
            feed_report.feed_sha256
        );
        assert_eq!(snapshot_report.p2p_ready_count, 0);
        assert_eq!(snapshot_report.peer_source_count, 0);
        assert!(snapshot_text.contains("\"peer_source_announcement_routes\""));
        assert!(snapshot_text.contains("\"route\": \"peer-source-announcements.v0.json\""));
        assert!(snapshot_text.contains(&format!("\"sha256\": \"{}\"", feed_report.feed_sha256)));
        assert!(snapshot_text
            .contains("\"route_kind\": \"minimodel.peer_source_announcement_catalog\""));
        assert!(snapshot_text.contains("\"peer_source_count\": 0"));
        assert!(!snapshot_text.contains("\"peer_sources\": ["));
        assert!(!snapshot_text.contains("artifact_project_server_url\": \"http"));
    }

    #[test]
    fn writes_empty_peer_source_announcement_feed_and_catalog_route() {
        let dir = unique_dir("empty-peer-source-announcement-feed");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let feed_path = dir.join("peer-source-announcements.v0.json");
        let catalog_snapshot_path = dir.join("models.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            Some("example-org/example-source"),
            None,
        )
        .unwrap();

        let feed_report = write_peer_source_announcement_feed_path(&feed_path, &[]).unwrap();
        let feed_text = std::fs::read_to_string(&feed_path).unwrap();
        assert_eq!(feed_report.announcement_count, 0);
        assert_eq!(feed_report.model_count, 0);
        assert_eq!(
            feed_report.feed_sha256,
            sha256_prefixed(feed_text.as_bytes())
        );
        assert!(feed_text.contains("\"announcements\": []"));

        let snapshot_report = write_catalog_snapshot_with_announcement_feed_path(
            &catalog_snapshot_path,
            "peer-source-announcements.v0.json",
            &feed_path,
            std::slice::from_ref(&catalog_entry_path),
        )
        .unwrap();
        let snapshot_text = std::fs::read_to_string(&catalog_snapshot_path).unwrap();

        assert_eq!(snapshot_report.entry_count, 1);
        assert_eq!(snapshot_report.announcement_count, 0);
        assert_eq!(snapshot_report.announcement_model_count, 0);
        assert_eq!(snapshot_report.p2p_ready_count, 0);
        assert_eq!(snapshot_report.peer_source_count, 0);
        assert!(snapshot_text.contains("\"peer_source_announcement_routes\""));
        assert!(snapshot_text.contains("\"route\": \"peer-source-announcements.v0.json\""));
        assert!(snapshot_text.contains(&format!("\"sha256\": \"{}\"", feed_report.feed_sha256)));
        assert!(snapshot_text.contains("\"peer_source_count\": 0"));
        assert!(!snapshot_text.contains("\"peer_sources\": ["));
    }

    #[test]
    fn rejects_announcement_feed_route_snapshot_unmatched_model() {
        let dir = unique_dir("peer-source-announcement-feed-unmatched");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let announcement_path = dir.join("tiny-test-model-q8.peer-source-announcement.v0.json");
        let feed_path = dir.join("peer-source-announcements.v0.json");
        let catalog_snapshot_path = dir.join("models.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            None,
        )
        .unwrap();
        write_peer_source_announcement_path(
            &catalog_entry_path,
            &announcement_path,
            "http://127.0.0.1:8123/peer-pieces",
        )
        .unwrap();
        let mut value: Value =
            serde_json::from_str(&std::fs::read_to_string(&announcement_path).unwrap()).unwrap();
        value.as_object_mut().unwrap().insert(
            "model_id".to_string(),
            Value::String("other/example-model".to_string()),
        );
        std::fs::write(
            &announcement_path,
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();
        write_peer_source_announcement_feed_path(
            &feed_path,
            std::slice::from_ref(&announcement_path),
        )
        .unwrap();

        let error = write_catalog_snapshot_with_announcement_feed_path(
            &catalog_snapshot_path,
            "peer-source-announcements.v0.json",
            &feed_path,
            &[catalog_entry_path],
        )
        .unwrap_err();

        assert!(error.contains("did not match any catalog entry"));
        assert!(!catalog_snapshot_path.exists());
    }

    #[test]
    fn rejects_duplicate_peer_source_announcement_feed_rows() {
        let dir = unique_dir("peer-source-announcement-feed-duplicate");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let announcement_path = dir.join("tiny-test-model-q8.peer-source-announcement.v0.json");
        let feed_path = dir.join("peer-source-announcements.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            None,
        )
        .unwrap();
        write_peer_source_announcement_path(
            &catalog_entry_path,
            &announcement_path,
            "http://127.0.0.1:8123/peer-pieces",
        )
        .unwrap();

        let error = write_peer_source_announcement_feed_path(
            &feed_path,
            &[announcement_path.clone(), announcement_path],
        )
        .unwrap_err();

        assert!(error.contains("duplicated model_id and peer_base_url"));
        assert!(!feed_path.exists());
    }

    #[test]
    fn rejects_unsafe_peer_source_announcement_feed_route() {
        let dir = unique_dir("peer-source-announcement-feed-route");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let announcement_path = dir.join("tiny-test-model-q8.peer-source-announcement.v0.json");
        let feed_path = dir.join("peer-source-announcements.v0.json");
        let catalog_snapshot_path = dir.join("models.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            None,
        )
        .unwrap();
        write_peer_source_announcement_path(
            &catalog_entry_path,
            &announcement_path,
            "http://127.0.0.1:8123/peer-pieces",
        )
        .unwrap();
        write_peer_source_announcement_feed_path(
            &feed_path,
            std::slice::from_ref(&announcement_path),
        )
        .unwrap();

        let error = write_catalog_snapshot_with_announcement_feed_path(
            &catalog_snapshot_path,
            "https://minimodel.org/peer-source-announcements.v0.json",
            &feed_path,
            &[catalog_entry_path],
        )
        .unwrap_err();

        assert!(error.contains("same-origin path metadata"));
        assert!(!catalog_snapshot_path.exists());
    }

    #[test]
    fn rejects_peer_source_announcement_artifact_drift() {
        let dir = unique_dir("peer-source-announcement-drift");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let announcement_path = dir.join("tiny-test-model-q8.peer-source-announcement.v0.json");
        let merged_entry_path = dir.join("tiny-test-model-q8.catalog-entry.announced-peer.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            None,
        )
        .unwrap();
        write_peer_source_announcement_path(
            &catalog_entry_path,
            &announcement_path,
            "http://127.0.0.1:8123/peer-pieces",
        )
        .unwrap();

        let mut value: Value =
            serde_json::from_str(&std::fs::read_to_string(&announcement_path).unwrap()).unwrap();
        value.as_object_mut().unwrap().insert(
            "artifact_sha256".to_string(),
            Value::String(
                "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                    .to_string(),
            ),
        );
        std::fs::write(
            &announcement_path,
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();

        let error = apply_peer_source_announcements_path(
            &catalog_entry_path,
            &merged_entry_path,
            &[announcement_path],
        )
        .unwrap_err();

        assert!(error.contains("artifact_sha256 mismatch"));
        assert!(!merged_entry_path.exists());
    }

    #[test]
    fn rejects_peer_source_announcement_peer_offer_route_drift() {
        let dir = unique_dir("peer-source-announcement-route-drift");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let announcement_path = dir.join("tiny-test-model-q8.peer-source-announcement.v0.json");
        let merged_entry_path = dir.join("tiny-test-model-q8.catalog-entry.announced-peer.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            None,
        )
        .unwrap();
        write_peer_source_announcement_path(
            &catalog_entry_path,
            &announcement_path,
            "http://127.0.0.1:8123/peer-pieces",
        )
        .unwrap();

        let mut value: Value =
            serde_json::from_str(&std::fs::read_to_string(&announcement_path).unwrap()).unwrap();
        value.as_object_mut().unwrap().insert(
            "peer_offer_route".to_string(),
            Value::String("different.minimodel-peer-offer".to_string()),
        );
        std::fs::write(
            &announcement_path,
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();

        let error = apply_peer_source_announcements_path(
            &catalog_entry_path,
            &merged_entry_path,
            &[announcement_path],
        )
        .unwrap_err();

        assert!(error.contains("peer_offer_route"));
        assert!(!merged_entry_path.exists());
    }

    #[test]
    fn rejects_peer_source_announcement_peer_offer_chunk_count_drift() {
        let dir = unique_dir("peer-source-announcement-chunk-count-drift");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let announcement_path = dir.join("tiny-test-model-q8.peer-source-announcement.v0.json");
        let merged_entry_path = dir.join("tiny-test-model-q8.catalog-entry.announced-peer.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            None,
        )
        .unwrap();
        write_peer_source_announcement_path(
            &catalog_entry_path,
            &announcement_path,
            "http://127.0.0.1:8123/peer-pieces",
        )
        .unwrap();

        let mut value: Value =
            serde_json::from_str(&std::fs::read_to_string(&announcement_path).unwrap()).unwrap();
        value.as_object_mut().unwrap().insert(
            "peer_offer_chunk_count".to_string(),
            Value::Number(99.into()),
        );
        std::fs::write(
            &announcement_path,
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();

        let error = apply_peer_source_announcements_path(
            &catalog_entry_path,
            &merged_entry_path,
            &[announcement_path],
        )
        .unwrap_err();

        assert!(error.contains("peer_offer_chunk_count"));
        assert!(!merged_entry_path.exists());
    }

    #[test]
    fn rejects_catalog_snapshot_with_unmatched_peer_source_announcement() {
        let dir = unique_dir("peer-source-announcement-unmatched");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let announcement_path = dir.join("tiny-test-model-q8.peer-source-announcement.v0.json");
        let catalog_snapshot_path = dir.join("models.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            None,
        )
        .unwrap();
        write_peer_source_announcement_path(
            &catalog_entry_path,
            &announcement_path,
            "http://127.0.0.1:8123/peer-pieces",
        )
        .unwrap();

        let mut value: Value =
            serde_json::from_str(&std::fs::read_to_string(&announcement_path).unwrap()).unwrap();
        value.as_object_mut().unwrap().insert(
            "model_id".to_string(),
            Value::String("other/example-model".to_string()),
        );
        std::fs::write(
            &announcement_path,
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();

        let error = write_catalog_snapshot_with_announcements_path(
            &catalog_snapshot_path,
            &[catalog_entry_path],
            &[announcement_path],
        )
        .unwrap_err();

        assert!(error.contains("did not match any catalog entry"));
        assert!(!catalog_snapshot_path.exists());
    }

    #[test]
    fn writes_catalog_snapshot_from_verified_entries() {
        let dir = unique_dir("catalog-snapshot");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let catalog_snapshot_path = dir.join("models.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            Some("example-org/example-source"),
            Some("http://127.0.0.1:8123/peer-pieces"),
        )
        .unwrap();

        let report =
            write_catalog_snapshot_path(&catalog_snapshot_path, &[catalog_entry_path.clone()])
                .unwrap();
        let text = std::fs::read_to_string(&catalog_snapshot_path).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();

        assert_eq!(report.entry_count, 1);
        assert_eq!(report.p2p_ready_count, 1);
        assert_eq!(report.peer_source_count, 1);
        assert_eq!(
            value["catalog_kind"].as_str(),
            Some("minimodel.model_catalog")
        );
        assert_eq!(value["entries"].as_array().unwrap().len(), 1);
        assert!(text.contains("\"source_huggingface_model_id\": \"example-org/example-source\""));
        assert!(text.contains("\"peer_base_url\": \"http://127.0.0.1:8123/peer-pieces/\""));
        assert!(text.contains("\"project_server_serves_model_bytes\": false"));
        assert!(!text.contains("artifact_project_server_url\": \"http"));
    }

    #[test]
    fn rejects_catalog_snapshot_duplicate_model_id() {
        let dir = unique_dir("catalog-snapshot-duplicate");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            None,
        )
        .unwrap();

        let error = write_catalog_snapshot_path(
            &dir.join("models.v0.json"),
            &[catalog_entry_path.clone(), catalog_entry_path],
        )
        .unwrap_err();

        assert!(error.contains("duplicate catalog model_id"));
        assert!(!dir.join("models.v0.json").exists());
    }

    #[test]
    fn rejects_catalog_snapshot_artifact_url() {
        let dir = unique_dir("catalog-snapshot-artifact-url");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            None,
        )
        .unwrap();
        let drifted = std::fs::read_to_string(&catalog_entry_path)
            .unwrap()
            .replace(
                "\"artifact_project_server_url\": null",
                "\"artifact_project_server_url\": \"https://MiniModel.org/models/tiny-test-model-q8.slm\"",
            );
        std::fs::write(&catalog_entry_path, drifted).unwrap();

        let error = write_catalog_snapshot_path(&dir.join("models.v0.json"), &[catalog_entry_path])
            .unwrap_err();

        assert!(error.contains("artifact_project_server_url"));
        assert!(error.contains("metadata only"));
    }

    #[test]
    fn rejects_catalog_snapshot_incomplete_peer_offer_proof() {
        let dir = unique_dir("catalog-snapshot-incomplete-peer-proof");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        let catalog_snapshot_path = dir.join("models.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            None,
        )
        .unwrap();

        let mut value: Value =
            serde_json::from_str(&std::fs::read_to_string(&catalog_entry_path).unwrap()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .remove("peer_offer_merkle_root_sha256");
        std::fs::write(
            &catalog_entry_path,
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();

        let error =
            write_catalog_snapshot_path(&catalog_snapshot_path, &[catalog_entry_path]).unwrap_err();

        assert!(error.contains("peer_offer_merkle_root_sha256"));
        assert!(!catalog_snapshot_path.exists());
    }

    #[test]
    fn rejects_catalog_snapshot_project_server_peer_source() {
        let dir = unique_dir("catalog-snapshot-project-peer");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_entry_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();
        write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_entry_path,
            None,
            Some("http://127.0.0.1:8123/peer-pieces"),
        )
        .unwrap();
        let drifted = std::fs::read_to_string(&catalog_entry_path)
            .unwrap()
            .replace(
                "\"peer_base_url\": \"http://127.0.0.1:8123/peer-pieces/\"",
                "\"peer_base_url\": \"http://MiniModel.org/peer-pieces/\"",
            );
        std::fs::write(&catalog_entry_path, drifted).unwrap();

        let error = write_catalog_snapshot_path(&dir.join("models.v0.json"), &[catalog_entry_path])
            .unwrap_err();

        assert!(error.contains("project server"));
        assert!(error.contains("never serve model bytes"));
    }

    #[test]
    fn parent_dir_helper_accepts_current_directory_outputs() {
        let dir = unique_dir("parent-dir-helper");
        let _ = std::fs::remove_dir_all(&dir);
        let nested_output = dir.join("nested").join("models.v0.json");

        ensure_parent_dir(Path::new("models.v0.json"), "catalog snapshot").unwrap();
        ensure_parent_dir(&nested_output, "catalog snapshot").unwrap();

        assert!(dir.join("nested").is_dir());
    }

    #[test]
    fn prepares_peer_share_from_local_slm_when_fixture_present() {
        let Some(artifact_path) = sibling_q8_fixture() else {
            eprintln!("skipping peer share preparation test; TinyRustLM fixture not present");
            return;
        };
        let dir = unique_dir("prepare-peer-share");
        let _ = std::fs::remove_dir_all(&dir);

        let report = prepare_peer_share_path(
            &artifact_path,
            &dir,
            "tinyrustlm/prepared-tiny-test-model-q8",
            Some("ExampleOrg/Tiny-Q8-Source"),
            Some("http://127.0.0.1:8123/peer-pieces"),
        )
        .unwrap();

        assert_eq!(report.model_id, "tinyrustlm/prepared-tiny-test-model-q8");
        assert_eq!(report.piece_count, 1);
        assert_eq!(report.peer_source_count, 1);
        assert_eq!(
            report.artifact_sha256,
            "sha256:6af985853325eaa6d3eaaa8aadd3511da9596eb542fb2d03a9bc9dc1f4377adf"
        );
        let manifest_path = Path::new(&report.manifest_path);
        let peer_offer_path = Path::new(&report.peer_offer_path);
        let catalog_entry_path = Path::new(&report.catalog_entry_path);
        let piece_dir_path = Path::new(&report.piece_dir_path);
        assert!(manifest_path.exists());
        assert!(peer_offer_path.exists());
        assert!(catalog_entry_path.exists());
        assert!(piece_dir_path.join("minimodel.peer_piece_set.v0").exists());

        let manifest = std::fs::read_to_string(manifest_path).unwrap();
        assert!(manifest.contains("artifact.acquisition=user-external-download"));
        assert!(manifest.contains("source.kind=safetensors"));
        assert!(manifest.contains("source.huggingface_model_id=ExampleOrg/Tiny-Q8-Source"));
        assert!(manifest.contains("chunks.mode=fixed-size-merkle-v0"));
        verify_artifact_path(manifest_path, &artifact_path).unwrap();
        inspect_peer_piece_dir_path(manifest_path, piece_dir_path).unwrap();

        let catalog = std::fs::read_to_string(catalog_entry_path).unwrap();
        assert!(catalog.contains("\"source_huggingface_model_id\": \"ExampleOrg/Tiny-Q8-Source\""));
        assert!(catalog.contains("\"peer_source_count\": 1"));
        assert!(catalog.contains("\"peer_base_url\": \"http://127.0.0.1:8123/peer-pieces/\""));
        assert!(!catalog.contains("artifact_project_server_url\": \"http"));
    }

    #[test]
    fn rejects_prepare_peer_share_project_server_peer_before_output() {
        let dir = unique_dir("prepare-peer-share-project-host");
        let _ = std::fs::remove_dir_all(&dir);

        let error = prepare_peer_share_path(
            Path::new("missing.slm"),
            &dir,
            "tinyrustlm/prepared-tiny-test-model-q8",
            None,
            Some("http://MiniModel.org/peer-pieces"),
        )
        .unwrap_err();

        assert!(error.contains("project server"));
        assert!(!dir.exists());
    }

    #[test]
    fn rejects_catalog_entry_peer_offer_drift() {
        let dir = unique_dir("catalog-entry-drift");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(
            &peer_offer_path,
            REAL_Q8_PEER_OFFER.replace("chunk_count=1", "chunk_count=2"),
        )
        .unwrap();

        let error =
            write_catalog_entry_path(&manifest_path, &peer_offer_path, &catalog_path, None, None)
                .unwrap_err();

        assert!(error.contains("peer offer chunk_count mismatch"));
        assert!(!catalog_path.exists());
    }

    #[test]
    fn rejects_catalog_entry_project_server_peer_source() {
        let dir = unique_dir("catalog-entry-project-peer");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("tiny-test-model-q8.minimodel.manifest.v0");
        let peer_offer_path = dir.join("tiny-test-model-q8.minimodel-peer-offer");
        let catalog_path = dir.join("tiny-test-model-q8.catalog-entry.v0.json");
        std::fs::write(&manifest_path, REAL_Q8_MANIFEST).unwrap();
        std::fs::write(&peer_offer_path, REAL_Q8_PEER_OFFER).unwrap();

        let error = write_catalog_entry_path(
            &manifest_path,
            &peer_offer_path,
            &catalog_path,
            None,
            Some("http://MiniModel.org/peer-pieces/"),
        )
        .unwrap_err();

        assert!(error.contains("project server"));
        assert!(!catalog_path.exists());
    }

    #[test]
    fn rejects_project_server_peer_piece_hosts_before_manifest_read() {
        let error = fetch_peer_pieces_http_path(
            Path::new("missing-manifest"),
            "http://MiniModel.org/peer-pieces/",
            Path::new("unused-output"),
        )
        .unwrap_err();
        assert!(error.contains("project server"));
        assert!(error.contains("never serve model bytes"));
    }

    #[test]
    fn rejects_https_peer_piece_url_in_first_prototype() {
        let error = fetch_peer_pieces_http_path(
            Path::new("missing-manifest"),
            "https://example.com/peer-pieces/",
            Path::new("unused-output"),
        )
        .unwrap_err();
        assert!(error.contains("http://"));
    }

    fn peer_import_request_json(peer_url: &str) -> String {
        REAL_Q8_PEER_IMPORT_REQUEST.replace("http://127.0.0.1:8123/", peer_url)
    }

    fn sibling_q8_fixture() -> Option<PathBuf> {
        let rust_root = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3)?;
        let path = rust_root
            .join("SLMComposer.com")
            .join("tinyrustlm")
            .join("models")
            .join("tiny-test-model-q8.slm");
        path.exists().then_some(path)
    }

    fn unique_dir(label: &str) -> PathBuf {
        let thread_label = std::thread::current()
            .name()
            .unwrap_or("thread")
            .replace(|ch: char| !ch.is_ascii_alphanumeric(), "-");
        std::env::temp_dir().join(format!(
            "minimodel-{label}-{}-{thread_label}",
            std::process::id()
        ))
    }

    fn start_peer_piece_server(root: PathBuf, request_count: usize) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            serve_peer_piece_dir_http_listener(&root, listener, Some(request_count)).unwrap();
        });
        format!("http://{address}/")
    }

    fn start_peer_status_server(
        manifest_path: PathBuf,
        piece_dir: PathBuf,
        request_count: usize,
    ) -> String {
        start_peer_status_server_with_path(manifest_path, piece_dir, "", request_count)
    }

    fn start_peer_status_server_with_path(
        manifest_path: PathBuf,
        piece_dir: PathBuf,
        path_prefix: &str,
        request_count: usize,
    ) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let normalized_prefix = path_prefix.trim_matches('/');
        let peer_url = if normalized_prefix.is_empty() {
            format!("http://{address}/")
        } else {
            format!("http://{address}/{normalized_prefix}/")
        };
        let peer_url_for_thread = peer_url.clone();
        std::thread::spawn(move || {
            serve_peer_pieces_http_listener(
                &manifest_path,
                &piece_dir,
                listener,
                Some(request_count),
                &peer_url_for_thread,
            )
            .unwrap();
        });
        peer_url
    }

    fn start_peer_companion_server(
        state_path: PathBuf,
        manifest_path: PathBuf,
        piece_dir: PathBuf,
        request_count: usize,
    ) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            serve_peer_companion_http_listener(
                &state_path,
                &manifest_path,
                &piece_dir,
                listener,
                Some(request_count),
            )
            .unwrap();
        });
        format!("http://{address}/")
    }

    fn start_peer_companion_server_for_share(
        manifest_path: PathBuf,
        piece_dir: PathBuf,
        state_path: PathBuf,
        request_count: usize,
    ) -> String {
        start_peer_companion_server_for_share_with_path(
            manifest_path,
            piece_dir,
            state_path,
            "",
            request_count,
        )
    }

    fn start_peer_companion_server_for_share_with_path(
        manifest_path: PathBuf,
        piece_dir: PathBuf,
        state_path: PathBuf,
        path_prefix: &str,
        request_count: usize,
    ) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let normalized_prefix = path_prefix.trim_matches('/');
        let peer_url = if normalized_prefix.is_empty() {
            format!("http://{address}/")
        } else {
            format!("http://{address}/{normalized_prefix}/")
        };
        write_peer_companion_state_path(&manifest_path, &piece_dir, &state_path, &peer_url)
            .unwrap();
        std::thread::spawn(move || {
            serve_peer_companion_http_listener(
                &state_path,
                &manifest_path,
                &piece_dir,
                listener,
                Some(request_count),
            )
            .unwrap();
        });
        peer_url
    }
}
