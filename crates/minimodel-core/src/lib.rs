mod manifest;
mod sha256;
mod slm;

use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub use manifest::Manifest;
pub use slm::SlmReport;

use sha256::{bytes_to_hex, normalize_sha256, parse_sha256_hex, sha256, sha256_prefixed};

const MINIMODEL_MANIFEST_SCHEMA_CHECKSUM: &str =
    "sha256:05d9dbd4f31ba6affeb9b4b70672636c296f9d584056f7add9ccc0fc02153cf4";
const DEFAULT_PEER_CHUNK_SIZE: usize = 1_048_576;

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
    pub piece_count: usize,
    pub manifest_sha256: String,
    pub artifact_sha256: String,
    pub merkle_root_sha256: String,
    pub peer_base_url: String,
}

#[derive(Debug, Clone)]
pub struct PeerPieceServe {
    pub piece_set_path: String,
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
pub struct PeerImportRequestExecution {
    pub request_path: String,
    pub peer_base_url: String,
    pub piece_dir_path: String,
    pub output_artifact_path: String,
    pub receipt_path: String,
    pub piece_count: usize,
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
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create receipt directory: {error}"))?;
    }
    fs::write(receipt_path, receipt)
        .map_err(|error| format!("failed to write import receipt: {error}"))?;
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
    if let Some(parent) = output_artifact_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create output directory: {error}"))?;
    }
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
    let piece_set_text = std::str::from_utf8(&piece_set_bytes)
        .map_err(|_| "peer piece set must be valid UTF-8".to_string())?;
    let piece_set = Manifest::parse(piece_set_text)?;
    validate_piece_set(&bundle.manifest, &bundle.bytes, &piece_set)?;
    let chunk_count = parse_usize(piece_set.require("chunks.count")?, "chunks.count")?;
    let temp_dir = temp_download_dir(output_dir)?;
    if temp_dir.exists() {
        return Err(format!(
            "temporary peer piece directory already exists: {}",
            temp_dir.display()
        ));
    }
    if let Some(parent) = output_dir.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create peer piece output parent: {error}"))?;
    }
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
            piece_count: chunk_count,
            manifest_sha256: sha256_prefixed(&bundle.bytes),
            artifact_sha256: normalize_sha256(bundle.manifest.require("artifact.sha256")?)?,
            merkle_root_sha256: normalize_sha256(
                bundle.manifest.require("chunks.merkle_root_sha256")?,
            )?,
            peer_base_url: base_url.to_string(),
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
    write_import_receipt_path(manifest_path, &output_artifact_path, &receipt_path)?;
    Ok(PeerImportRequestExecution {
        request_path: request_path.display().to_string(),
        peer_base_url: fetch.peer_base_url,
        piece_dir_path: piece_dir.display().to_string(),
        output_artifact_path: output_artifact_path.display().to_string(),
        receipt_path: receipt_path.display().to_string(),
        piece_count: fetch.piece_count,
        manifest_sha256: artifact.manifest_sha256,
        artifact_sha256: artifact.artifact_sha256,
        merkle_root_sha256: fetch.merkle_root_sha256,
    })
}

pub fn inspect_peer_piece_dir_path(
    manifest_path: &Path,
    piece_dir: &Path,
) -> Result<PeerPieceServe, String> {
    let bundle = read_valid_manifest_path(manifest_path)?;
    require_fixed_chunk_mode(&bundle.manifest)?;
    let piece_set_path = piece_dir.join("minimodel.peer_piece_set.v0");
    let piece_set_text = fs::read_to_string(&piece_set_path)
        .map_err(|error| format!("failed to read peer piece set: {error}"))?;
    let piece_set = Manifest::parse(&piece_set_text)?;
    validate_piece_set(&bundle.manifest, &bundle.bytes, &piece_set)?;
    let chunk_count = validate_peer_piece_files(&piece_set, piece_dir)?;
    Ok(PeerPieceServe {
        piece_set_path: piece_set_path.display().to_string(),
        piece_count: chunk_count,
        manifest_sha256: sha256_prefixed(&bundle.bytes),
        artifact_sha256: normalize_sha256(bundle.manifest.require("artifact.sha256")?)?,
        merkle_root_sha256: normalize_sha256(
            bundle.manifest.require("chunks.merkle_root_sha256")?,
        )?,
    })
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
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create catalog entry directory: {error}"))?;
    }
    fs::write(output_path, catalog_entry)
        .map_err(|error| format!("failed to write catalog entry: {error}"))?;
    Ok(CatalogEntryReport {
        catalog_entry_path: output_path.display().to_string(),
        model_id: bundle.manifest.require("model.id")?.to_string(),
        artifact_sha256: normalize_sha256(bundle.manifest.require("artifact.sha256")?)?,
        peer_offer_sha256,
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
    let report = inspect_peer_piece_dir_path(manifest_path, piece_dir)?;
    serve_peer_piece_dir_http_path(piece_dir, bind_addr, None)?;
    Ok(report)
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
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read peer import request: {error}"))?;
    let request_file: PeerImportRequestFile = serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse peer import request JSON: {error}"))?;
    let request = PeerImportRequest::from(request_file);
    validate_peer_import_request_static_fields(&request)?;
    Ok(request)
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
    let fields = [
        json_string_field("source_kind", "known-direct-http-peer-v0"),
        json_string_field("transport", "direct-http-piece-server-v0"),
        json_string_field("peer_base_url", peer_base_url),
        json_string_field("consent_status", "publisher-declared-consenting-peer"),
        json_bool_field("project_server_serves_model_bytes", false),
        json_bool_field("project_server_runs_model_bytes", false),
    ];
    format!("[{{{}}}]", fields.join(", "))
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
    let mut text = String::new();
    push_line(&mut text, "piece_set.version", "0");
    push_line(&mut text, "piece_set.kind", "minimodel.peer_piece_set.v0");
    push_line(&mut text, "manifest.sha256", &report.manifest_sha256);
    push_line(&mut text, "manifest.model_id", &report.manifest_model_id);
    push_line(
        &mut text,
        "artifact.byte_count",
        &report.artifact_byte_count.to_string(),
    );
    push_line(&mut text, "artifact.sha256", &report.artifact_sha256);
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

fn serve_peer_piece_dir_http_path(
    piece_dir: &Path,
    bind_addr: &str,
    request_limit: Option<usize>,
) -> Result<(), String> {
    let listener = TcpListener::bind(bind_addr)
        .map_err(|error| format!("failed to bind peer piece server at {bind_addr}: {error}"))?;
    serve_peer_piece_dir_http_listener(piece_dir, listener, request_limit)
}

fn serve_peer_piece_dir_http_listener(
    piece_dir: &Path,
    listener: TcpListener,
    request_limit: Option<usize>,
) -> Result<(), String> {
    let piece_set_text = fs::read_to_string(piece_dir.join("minimodel.peer_piece_set.v0"))
        .map_err(|error| format!("failed to read peer piece set: {error}"))?;
    let piece_set = Manifest::parse(&piece_set_text)?;
    let allowed_names = allowed_peer_piece_names(&piece_set)?;
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
        if let Err(error) = serve_peer_piece_request(&mut stream, piece_dir, &allowed_names) {
            let _ = write_http_response(&mut stream, "400 Bad Request", error.as_bytes());
        }
    }
}

fn allowed_peer_piece_names(piece_set: &Manifest) -> Result<BTreeSet<String>, String> {
    let chunk_count = parse_usize(piece_set.require("chunks.count")?, "chunks.count")?;
    let mut names = BTreeSet::new();
    names.insert("minimodel.peer_piece_set.v0".to_string());
    for index in 0..chunk_count {
        names.insert(peer_piece_spec(piece_set, index)?.file_name);
    }
    Ok(names)
}

fn serve_peer_piece_request(
    stream: &mut TcpStream,
    piece_dir: &Path,
    allowed_names: &BTreeSet<String>,
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
    let file_name = path.trim_start_matches('/');
    if file_name.is_empty() || require_safe_piece_file_name(file_name).is_err() {
        write_http_response(stream, "400 Bad Request", b"bad peer piece path")?;
        return Ok(());
    }
    if !allowed_names.contains(file_name) {
        write_http_response(stream, "404 Not Found", b"not found")?;
        return Ok(());
    }
    let bytes = fs::read(piece_dir.join(file_name))
        .map_err(|error| format!("failed to read served peer piece: {error}"))?;
    write_http_response(stream, "200 OK", &bytes)
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

impl std::fmt::Display for HttpUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.port == 80 {
            write!(formatter, "http://{}{}", self.host, self.path)
        } else {
            write!(formatter, "http://{}:{}{}", self.host, self.port, self.path)
        }
    }
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
    ];
    if forbidden.contains(&lower.as_str())
        || lower.ends_with(".minimodel.org")
        || lower.ends_with(".tinyrustlm.com")
        || lower.ends_with(".slmcomposer.com")
        || lower.ends_with(".lmcomposer.com")
        || lower.ends_with(".mirust.com")
    {
        return Err(format!(
            "peer piece URL host {host} is a project server; project servers never serve model bytes"
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

fn format_unix_utc(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
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
        assert!(error.contains("piece.0.sha256"));
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
        let peer_url = start_peer_piece_server(served_piece_dir, 2);
        let fetched_piece_dir = dir.join("fetched-pieces");
        let fetch = fetch_peer_pieces_http_path(&manifest_path, &peer_url, &fetched_piece_dir)
            .expect("HTTP peer fetch should pass");
        assert_eq!(fetch.piece_count, 1);
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
        let peer_url = start_peer_piece_server(served_piece_dir, 2);
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        std::fs::write(&request_path, peer_import_request_json(&peer_url)).unwrap();
        let output_dir = dir.join("imported");

        let report =
            execute_peer_import_request_path(&request_path, &manifest_path, &output_dir).unwrap();

        assert_eq!(report.peer_base_url, peer_url);
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
        let peer_url = start_peer_piece_server(served_piece_dir, 2);
        let fetched_piece_dir = dir.join("fetched-pieces");
        let error =
            fetch_peer_pieces_http_path(&manifest_path, &peer_url, &fetched_piece_dir).unwrap_err();
        assert!(error.contains("piece.0.sha256"));
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
        let request_path = dir.join("tiny-test-model-q8.minimodel-peer-import-request.json");
        std::fs::write(
            &request_path,
            peer_import_request_json("http://MiniModel.org/peer-pieces/"),
        )
        .unwrap();
        let error = execute_peer_import_request_path(
            &request_path,
            Path::new("missing-manifest"),
            &dir.join("imported"),
        )
        .unwrap_err();
        assert!(error.contains("project server"));
        assert!(error.contains("never serve model bytes"));
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
}
