#![allow(unsafe_code)]
#![doc = "Narrow C ABI for managed MiniModel seed-host orchestration."]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::ptr;

const NATIVE_ABI_VERSION: u32 = 2;
const MAX_REQUEST_JSON_BYTES: usize = 4 * 1024 * 1024;

/// Returns the MiniModel native ABI version.
#[no_mangle]
pub extern "C" fn minimodel_native_abi_version() -> u32 {
    NATIVE_ABI_VERSION
}

/// Executes one allowlisted MiniModel operation and returns a JSON envelope.
///
/// # Safety
///
/// `request_json` must be a valid pointer to a null-terminated byte sequence
/// for the duration of the call. Invalid UTF-8 is rejected in the envelope.
#[no_mangle]
pub unsafe extern "C" fn minimodel_native_invoke_json(request_json: *const c_char) -> *mut c_char {
    let response = invoke_boundary(|| {
        if request_json.is_null() {
            return Err(NativeError::new(
                "invalid_argument",
                "request_json must not be null",
            ));
        }
        let request_bytes = CStr::from_ptr(request_json).to_bytes();
        invoke_request_bytes(request_bytes)
    });
    response_to_raw(response)
}

/// Releases a string returned by `minimodel_native_invoke_json`.
///
/// Passing a null pointer is allowed.
///
/// # Safety
///
/// `ptr` must be null or a pointer returned by this library that has not
/// already been released.
#[no_mangle]
pub unsafe extern "C" fn minimodel_native_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum NativeRequest {
    InspectManifest {
        manifest_path: String,
    },
    VerifyArtifact {
        manifest_path: String,
        artifact_path: String,
    },
    InspectPeerPieceDir {
        manifest_path: String,
        piece_dir: String,
    },
    InspectPeerCompanionState {
        state_path: String,
        manifest_path: String,
        piece_dir: String,
    },
    WritePeerCompanionState {
        manifest_path: String,
        piece_dir: String,
        output_path: String,
        known_peer_base_url: String,
    },
    InspectSeedFleet {
        config_path: String,
    },
    InspectExternalProbeReceipt {
        receipt_path: String,
        trust_path: String,
        config_path: String,
    },
    PrepareSeedAnnouncement {
        catalog_entry_path: String,
        output_path: String,
        receipt_path: String,
        trust_path: String,
        config_path: String,
    },
    PreparePeerShare {
        artifact_path: String,
        output_dir: String,
        model_id: String,
        source_huggingface_model_id: Option<String>,
        known_peer_base_url: Option<String>,
        provenance: Option<Box<minimodel_core::PeerShareProvenance>>,
    },
    VerifyLocalPeerRoundTrip {
        manifest_path: String,
        artifact_path: String,
        piece_dir: String,
        work_dir: String,
        receipt_path: String,
    },
    CheckPeerStatus {
        manifest_path: String,
        peer_base_url: String,
        output_path: String,
    },
    PreflightPeerImport {
        request_path: String,
        manifest_path: String,
        output_path: String,
    },
    ExecutePeerImport {
        request_path: String,
        manifest_path: String,
        output_dir: String,
    },
    InspectPeerImportReceipt {
        receipt_path: String,
        request_path: String,
        preflight_path: String,
        manifest_path: String,
        artifact_path: String,
    },
}

#[derive(Debug, Serialize)]
struct NativeEnvelope {
    ok: bool,
    value: Option<Value>,
    error: Option<NativeError>,
}

#[derive(Debug, Serialize)]
struct NativeError {
    code: &'static str,
    message: String,
}

impl NativeError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Validate input size and UTF-8 before parsing an ABI operation request.
fn invoke_request_bytes(request_bytes: &[u8]) -> Result<Value, NativeError> {
    if request_bytes.is_empty() || request_bytes.len() > MAX_REQUEST_JSON_BYTES {
        return Err(NativeError::new(
            "invalid_argument",
            format!("request_json must contain 1 to {MAX_REQUEST_JSON_BYTES} UTF-8 bytes"),
        ));
    }
    let request_text = std::str::from_utf8(request_bytes)
        .map_err(|_| NativeError::new("invalid_utf8", "request_json must be valid UTF-8"))?;
    let request: NativeRequest = serde_json::from_str(request_text)
        .map_err(|error| NativeError::new("invalid_json", error.to_string()))?;
    dispatch_request(request)
}

/// Dispatch only reviewed public MiniModel operations to the Rust authority.
fn dispatch_request(request: NativeRequest) -> Result<Value, NativeError> {
    let operation_result = match request {
        NativeRequest::InspectManifest { manifest_path } => {
            minimodel_core::inspect_manifest_path(Path::new(&manifest_path)).and_then(to_value)
        }
        NativeRequest::VerifyArtifact {
            manifest_path,
            artifact_path,
        } => minimodel_core::verify_artifact_path(
            Path::new(&manifest_path),
            Path::new(&artifact_path),
        )
        .and_then(to_value),
        NativeRequest::InspectPeerPieceDir {
            manifest_path,
            piece_dir,
        } => minimodel_core::inspect_peer_piece_dir_path(
            Path::new(&manifest_path),
            Path::new(&piece_dir),
        )
        .and_then(to_value),
        NativeRequest::InspectPeerCompanionState {
            state_path,
            manifest_path,
            piece_dir,
        } => minimodel_core::inspect_peer_companion_state_path(
            Path::new(&state_path),
            Path::new(&manifest_path),
            Path::new(&piece_dir),
        )
        .and_then(to_value),
        NativeRequest::WritePeerCompanionState {
            manifest_path,
            piece_dir,
            output_path,
            known_peer_base_url,
        } => minimodel_core::write_peer_companion_state_path(
            Path::new(&manifest_path),
            Path::new(&piece_dir),
            Path::new(&output_path),
            &known_peer_base_url,
        )
        .and_then(to_value),
        NativeRequest::InspectSeedFleet { config_path } => {
            minimodel_core::inspect_seed_fleet_path(Path::new(&config_path)).and_then(to_value)
        }
        NativeRequest::InspectExternalProbeReceipt {
            receipt_path,
            trust_path,
            config_path,
        } => minimodel_core::inspect_external_probe_receipt_path(
            Path::new(&receipt_path),
            Path::new(&trust_path),
            Path::new(&config_path),
        )
        .and_then(to_value),
        NativeRequest::PrepareSeedAnnouncement {
            catalog_entry_path,
            output_path,
            receipt_path,
            trust_path,
            config_path,
        } => minimodel_core::prepare_seed_peer_source_announcement_path(
            Path::new(&catalog_entry_path),
            Path::new(&output_path),
            Path::new(&receipt_path),
            Path::new(&trust_path),
            Path::new(&config_path),
        )
        .and_then(to_value),
        NativeRequest::PreparePeerShare {
            artifact_path,
            output_dir,
            model_id,
            source_huggingface_model_id,
            known_peer_base_url,
            provenance,
        } => minimodel_core::prepare_peer_share_with_provenance_path(
            Path::new(&artifact_path),
            Path::new(&output_dir),
            &model_id,
            source_huggingface_model_id.as_deref(),
            known_peer_base_url.as_deref(),
            provenance.as_deref(),
        )
        .and_then(to_value),
        NativeRequest::VerifyLocalPeerRoundTrip {
            manifest_path,
            artifact_path,
            piece_dir,
            work_dir,
            receipt_path,
        } => minimodel_core::verify_local_peer_round_trip_path(
            Path::new(&manifest_path),
            Path::new(&artifact_path),
            Path::new(&piece_dir),
            Path::new(&work_dir),
            Path::new(&receipt_path),
        )
        .and_then(to_value),
        NativeRequest::CheckPeerStatus {
            manifest_path,
            peer_base_url,
            output_path,
        } => minimodel_core::check_peer_status_path(
            Path::new(&manifest_path),
            &peer_base_url,
            Path::new(&output_path),
        )
        .and_then(to_value),
        NativeRequest::PreflightPeerImport {
            request_path,
            manifest_path,
            output_path,
        } => minimodel_core::preflight_peer_import_request_path(
            Path::new(&request_path),
            Path::new(&manifest_path),
            Path::new(&output_path),
        )
        .and_then(to_value),
        NativeRequest::ExecutePeerImport {
            request_path,
            manifest_path,
            output_dir,
        } => minimodel_core::execute_peer_import_request_path(
            Path::new(&request_path),
            Path::new(&manifest_path),
            Path::new(&output_dir),
        )
        .and_then(to_value),
        NativeRequest::InspectPeerImportReceipt {
            receipt_path,
            request_path,
            preflight_path,
            manifest_path,
            artifact_path,
        } => minimodel_core::inspect_peer_import_receipt_path(
            Path::new(&receipt_path),
            Path::new(&request_path),
            Path::new(&preflight_path),
            Path::new(&manifest_path),
            Path::new(&artifact_path),
        )
        .and_then(to_value),
    };
    operation_result.map_err(|message| NativeError::new("minimodel_operation_failed", message))
}

/// Serialize a validated report without exposing an unvalidated native pointer.
fn to_value<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| format!("failed to serialize report: {error}"))
}

/// Catch Rust panics and always return a bounded public-safe JSON envelope.
fn invoke_boundary<F>(operation: F) -> String
where
    F: FnOnce() -> Result<Value, NativeError>,
{
    let envelope = match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(value)) => NativeEnvelope {
            ok: true,
            value: Some(value),
            error: None,
        },
        Ok(Err(error)) => NativeEnvelope {
            ok: false,
            value: None,
            error: Some(error),
        },
        Err(_) => NativeEnvelope {
            ok: false,
            value: None,
            error: Some(NativeError::new(
                "native_panic",
                "MiniModel native operation terminated unexpectedly",
            )),
        },
    };
    serde_json::to_string(&envelope).unwrap_or_else(|_| {
        "{\"ok\":false,\"value\":null,\"error\":{\"code\":\"response_encoding_failed\",\"message\":\"MiniModel native response encoding failed\"}}".to_string()
    })
}

/// Transfer an encoded JSON response to the ABI caller.
fn response_to_raw(response: String) -> *mut c_char {
    CString::new(response)
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_rejects_null_invalid_utf8_and_oversize_requests() {
        unsafe {
            let null_response = minimodel_native_invoke_json(ptr::null());
            let null_text = take_response(null_response);
            assert!(null_text.contains("invalid_argument"));

            let invalid_utf8 = [0xffu8, 0];
            let invalid_response =
                minimodel_native_invoke_json(invalid_utf8.as_ptr().cast::<c_char>());
            let invalid_text = take_response(invalid_response);
            assert!(invalid_text.contains("invalid_utf8"));

            let mut oversized = vec![b' '; MAX_REQUEST_JSON_BYTES + 1];
            oversized.push(0);
            let oversized_response =
                minimodel_native_invoke_json(oversized.as_ptr().cast::<c_char>());
            let oversized_text = take_response(oversized_response);
            assert!(oversized_text.contains("invalid_argument"));

            minimodel_native_string_free(ptr::null_mut());
        }
    }

    #[test]
    fn abi_rejects_unknown_fields_and_operations() {
        let unknown_field = invoke_request_bytes(
            br#"{"operation":"inspect_seed_fleet","config_path":"fleet.json","extra":true}"#,
        )
        .unwrap_err();
        assert_eq!(unknown_field.code, "invalid_json");

        let unknown_operation =
            invoke_request_bytes(br#"{"operation":"private_conversion"}"#).unwrap_err();
        assert_eq!(unknown_operation.code, "invalid_json");
    }

    #[test]
    fn abi_allowlists_external_probe_verification_and_proof_gated_announcement() {
        let verification = invoke_request_bytes(
            br#"{"operation":"inspect_external_probe_receipt","receipt_path":"missing.receipt.json","trust_path":"missing.trust.json","config_path":"missing.fleet.json"}"#,
        )
        .unwrap_err();
        assert_eq!(verification.code, "minimodel_operation_failed");

        let announcement = invoke_request_bytes(
            br#"{"operation":"prepare_seed_announcement","catalog_entry_path":"missing.catalog.json","output_path":"missing.announcement.json","receipt_path":"missing.receipt.json","trust_path":"missing.trust.json","config_path":"missing.fleet.json"}"#,
        )
        .unwrap_err();
        assert_eq!(announcement.code, "minimodel_operation_failed");
    }

    #[test]
    fn abi_requires_trained_share_provenance_and_rejects_unknown_nested_fields() {
        let missing = invoke_request_bytes(
            br#"{"operation":"prepare_peer_share","artifact_path":"missing.slm","output_dir":"share","model_id":"tinyrustlm/trained","source_huggingface_model_id":"ExampleOrg/Trained","known_peer_base_url":null,"provenance":null}"#,
        )
        .unwrap_err();
        assert_eq!(missing.code, "minimodel_operation_failed");
        assert!(missing.message.contains("requires provenance"));

        let unknown = invoke_request_bytes(
            br#"{"operation":"prepare_peer_share","artifact_path":"missing.slm","output_dir":"share","model_id":"tinyrustlm/trained","source_huggingface_model_id":"ExampleOrg/Trained","known_peer_base_url":null,"provenance":{"source_revision":"0123456789abcdef0123456789abcdef01234567","source_license_id":"Apache-2.0","source_license_path":"LICENSE.txt","source_license_sha256":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","modification_notice_path":"NOTICE.md","modification_notice_sha256":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","conversation_evidence_path":"conversation.evidence","conversation_evidence_sha256":"sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","evaluation_evidence_path":"quality.eval","evaluation_evidence_sha256":"sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd","quality_tier":"conversation-proven-full-assistant-pending","known_limitation":"strict-json","unreviewed_claim":"excellent"}}"#,
        )
        .unwrap_err();
        assert_eq!(unknown.code, "invalid_json");
    }

    #[test]
    fn abi_allowlists_real_local_peer_round_trip_verification() {
        let verification = invoke_request_bytes(
            br#"{"operation":"verify_local_peer_round_trip","manifest_path":"missing.manifest","artifact_path":"missing.slm","piece_dir":"missing-pieces","work_dir":"round-trip-work","receipt_path":"round-trip.receipt"}"#,
        )
        .unwrap_err();

        assert_eq!(verification.code, "minimodel_operation_failed");
        assert!(verification.message.contains("manifest"));
    }

    #[test]
    fn abi_catches_panics_without_exposing_panic_payloads() {
        let response =
            invoke_boundary(|| -> Result<Value, NativeError> { panic!("sensitive panic payload") });
        assert!(response.contains("native_panic"));
        assert!(!response.contains("sensitive panic payload"));
    }

    unsafe fn take_response(ptr: *mut c_char) -> String {
        assert!(!ptr.is_null());
        let text = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        minimodel_native_string_free(ptr);
        text
    }
}
