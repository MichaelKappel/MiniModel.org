use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::Value;
use sha2::{Digest, Sha256};

mod common;

use common::{minimodel_bin, output_text, sibling_q8_fixture, unique_dir};

const MODEL_ID: &str = "tinyrustlm/seed-fleet-test-q8";

struct FleetFixture {
    root: PathBuf,
    config: PathBuf,
    lane_a_url: String,
    lane_b_url: String,
}

struct SeedLaneProcess {
    child: Child,
}

impl SeedLaneProcess {
    fn start(config: &Path, lane_id: &str) -> Self {
        let child = Command::new(minimodel_bin())
            .args(["serve-seed-lane", &config.to_string_lossy(), lane_id])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start seed lane");
        Self { child }
    }

    fn start_with_shutdown_file(config: &Path, lane_id: &str, shutdown_file: &Path) -> Self {
        let child = Command::new(minimodel_bin())
            .args([
                "serve-seed-lane",
                &config.to_string_lossy(),
                lane_id,
                &shutdown_file.to_string_lossy(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start seed lane with shutdown file");
        Self { child }
    }

    fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    fn stop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

impl Drop for SeedLaneProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Drop for FleetFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn inspect_seed_fleet_cli_requires_two_independent_lanes_and_twenty_model_capacity() {
    let Some(fixture) = build_fleet_fixture("inspect") else {
        return;
    };

    let output = Command::new(minimodel_bin())
        .args(["inspect-seed-fleet", &fixture.config.to_string_lossy()])
        .output()
        .expect("run inspect-seed-fleet");

    assert!(output.status.success(), "{}", output_text(&output));
    let text = output_text(&output);
    assert!(text.contains("MiniModel seed fleet inspection: PASS"));
    assert!(text.contains("fleet_id: test-fleet-inspect"));
    assert!(text.contains("target_model_count: 20"));
    assert!(text.contains("admitted_model_count: 1"));
    assert!(text.contains("active_model_count: 1"));
    assert!(text.contains("remaining_model_capacity: 19"));
    assert!(text.contains("replica_lane_count: 2"));
    assert!(text.contains("peer_endpoint_count: 2"));
    assert!(text.contains("advertisement_status: external-probe-required"));
    assert!(text.contains("project_server_serves_model_bytes: false"));
}

#[test]
fn inspect_seed_fleet_cli_preserves_disabled_models_without_serving_them() {
    let Some(fixture) = build_fleet_fixture("disabled") else {
        return;
    };
    let mut config: Value =
        serde_json::from_str(&std::fs::read_to_string(&fixture.config).expect("read fleet config"))
            .expect("parse fleet config");
    config["models"][0]["enabled"] = Value::Bool(false);
    std::fs::write(
        &fixture.config,
        serde_json::to_vec_pretty(&config).expect("serialize disabled fleet"),
    )
    .expect("write disabled fleet");

    let inspect = Command::new(minimodel_bin())
        .args(["inspect-seed-fleet", &fixture.config.to_string_lossy()])
        .output()
        .expect("inspect disabled fleet");
    assert!(inspect.status.success(), "{}", output_text(&inspect));
    let text = output_text(&inspect);
    assert!(text.contains("admitted_model_count: 1"), "{text}");
    assert!(text.contains("active_model_count: 0"), "{text}");
    assert!(text.contains("remaining_model_capacity: 19"), "{text}");
    assert!(text.contains("peer_endpoint_count: 0"), "{text}");

    let serve = Command::new(minimodel_bin())
        .args([
            "serve-seed-lane",
            &fixture.config.to_string_lossy(),
            "seed-a",
        ])
        .output()
        .expect("reject disabled-only lane");
    assert!(!serve.status.success(), "{}", output_text(&serve));
    assert!(
        output_text(&serve).contains("has no enabled endpoints"),
        "{}",
        output_text(&serve)
    );
}

#[test]
fn inspect_seed_fleet_cli_accepts_an_empty_initialized_fleet() {
    let Some(fixture) = build_fleet_fixture("empty") else {
        return;
    };
    let mut config: Value =
        serde_json::from_str(&std::fs::read_to_string(&fixture.config).expect("read fleet config"))
            .expect("parse fleet config");
    config["models"] = serde_json::json!([]);
    config["lanes"][0]["endpoints"] = serde_json::json!([]);
    config["lanes"][1]["endpoints"] = serde_json::json!([]);
    std::fs::write(
        &fixture.config,
        serde_json::to_vec_pretty(&config).expect("serialize empty fleet"),
    )
    .expect("write empty fleet");

    let output = Command::new(minimodel_bin())
        .args(["inspect-seed-fleet", &fixture.config.to_string_lossy()])
        .output()
        .expect("inspect empty fleet");
    assert!(output.status.success(), "{}", output_text(&output));
    let text = output_text(&output);
    assert!(text.contains("admitted_model_count: 0"), "{text}");
    assert!(text.contains("active_model_count: 0"), "{text}");
    assert!(text.contains("remaining_model_capacity: 20"), "{text}");
}

#[test]
fn inspect_external_probe_receipt_accepts_only_current_independently_signed_evidence() {
    let Some(fixture) = build_fleet_fixture("external-receipt") else {
        return;
    };
    let signed = write_external_probe_fixture(
        &fixture,
        "valid",
        -30,
        570,
        "outside-host-and-local-network",
        false,
    );

    let output = inspect_external_probe(&fixture, &signed.receipt_path, &signed.trust_path);
    assert!(output.status.success(), "{}", output_text(&output));
    let text = output_text(&output);
    assert!(
        text.contains("MiniModel external probe receipt inspection: PASS"),
        "{text}"
    );
    assert!(
        text.contains("observer_id: fixture-external-observer"),
        "{text}"
    );
    assert!(
        text.contains("fleet_id: test-fleet-external-receipt"),
        "{text}"
    );
    assert!(text.contains("lane_id: seed-a"), "{text}");
    assert!(text.contains("externally_reachable: true"), "{text}");
    assert!(text.contains("announcement_eligible: true"), "{text}");

    let catalog_entry = fixture
        .root
        .join("share/tiny-test-model-q8.catalog-entry.v0.json");
    let announcement_path = fixture.root.join("seed-a.announcement.v1.json");
    let prepared = Command::new(minimodel_bin())
        .args([
            "prepare-seed-announcement",
            &catalog_entry.to_string_lossy(),
            &announcement_path.to_string_lossy(),
            &signed.receipt_path.to_string_lossy(),
            &signed.trust_path.to_string_lossy(),
            &fixture.config.to_string_lossy(),
        ])
        .output()
        .expect("prepare proof-gated seed announcement");
    assert!(prepared.status.success(), "{}", output_text(&prepared));
    let prepared_text = output_text(&prepared);
    assert!(prepared_text.contains("proof-gated seed announcement prepared: PASS"));
    assert!(prepared_text.contains("advertised: false"));
    let announcement: Value = serde_json::from_slice(
        &std::fs::read(&announcement_path).expect("read proof-gated announcement"),
    )
    .expect("parse proof-gated announcement");
    assert_eq!(announcement["announcement_version"], 1);
    assert_eq!(
        announcement["announcement_status"],
        "peer-source-externally-verified-metadata-only"
    );
    assert_eq!(
        announcement["external_reachability"]["receipt_id"],
        "fixture-receipt-valid"
    );
    assert_eq!(
        announcement["external_reachability"]["network_scope"],
        "outside-host-and-local-network"
    );

    let replay_output = fixture.root.join("seed-a.replayed.announcement.v1.json");
    let replay = Command::new(minimodel_bin())
        .args([
            "prepare-seed-announcement",
            &catalog_entry.to_string_lossy(),
            &replay_output.to_string_lossy(),
            &signed.receipt_path.to_string_lossy(),
            &signed.trust_path.to_string_lossy(),
            &fixture.config.to_string_lossy(),
        ])
        .output()
        .expect("reject replayed external receipt");
    assert!(!replay.status.success(), "{}", output_text(&replay));
    assert!(
        output_text(&replay).contains("receipt replay rejected"),
        "{}",
        output_text(&replay)
    );
    assert!(!replay_output.exists());
    let replay_records = std::fs::read_dir(fixture.root.join("external-probe-replay-v0"))
        .expect("read replay state")
        .count();
    assert_eq!(replay_records, 1);

    let feed_path = fixture.root.join("verified-announcement-feed.json");
    let feed = Command::new(minimodel_bin())
        .args([
            "write-peer-source-announcement-feed",
            &feed_path.to_string_lossy(),
            &announcement_path.to_string_lossy(),
        ])
        .output()
        .expect("validate current proof-gated announcement in feed");
    assert!(feed.status.success(), "{}", output_text(&feed));

    let mut stale_announcement = announcement;
    stale_announcement["external_reachability"]["expires_utc"] =
        Value::String("2000-01-01T00:00:00Z".to_string());
    let stale_announcement_path = fixture.root.join("stale.announcement.v1.json");
    write_json(&stale_announcement_path, &stale_announcement);
    let stale_feed_path = fixture.root.join("stale-announcement-feed.json");
    let stale_feed = Command::new(minimodel_bin())
        .args([
            "write-peer-source-announcement-feed",
            &stale_feed_path.to_string_lossy(),
            &stale_announcement_path.to_string_lossy(),
        ])
        .output()
        .expect("reject stale proof-gated announcement");
    assert!(!stale_feed.status.success(), "{}", output_text(&stale_feed));
    assert!(
        output_text(&stale_feed).contains("stale or expired"),
        "{}",
        output_text(&stale_feed)
    );
    assert!(!stale_feed_path.exists());

    let expired = write_external_probe_fixture(
        &fixture,
        "expired",
        -1_200,
        -600,
        "outside-host-and-local-network",
        false,
    );
    assert_external_probe_rejected(&fixture, &expired, "receipt is expired");

    let local_only =
        write_external_probe_fixture(&fixture, "local-only", -30, 570, "same-host-only", false);
    assert_external_probe_rejected(
        &fixture,
        &local_only,
        "network_scope must be outside-host-and-local-network",
    );

    let revoked = write_external_probe_fixture(
        &fixture,
        "revoked",
        -30,
        570,
        "outside-host-and-local-network",
        true,
    );
    assert_external_probe_rejected(&fixture, &revoked, "observer key is revoked");

    let mut endpoint_drift = signed.receipt.clone();
    endpoint_drift["public_base_url"] = Value::String(fixture.lane_b_url.clone());
    let endpoint_drift_path = fixture.root.join("external-receipt-endpoint-drift.json");
    write_json(&endpoint_drift_path, &endpoint_drift);
    assert_external_probe_path_rejected(
        &fixture,
        &endpoint_drift_path,
        &signed.trust_path,
        "public_base_url mismatch",
    );

    let mut signature_drift = signed.receipt.clone();
    signature_drift["checked_routes"][0]["body_byte_count"] = Value::from(99_u64);
    let signature_drift_path = fixture.root.join("external-receipt-signature-drift.json");
    write_json(&signature_drift_path, &signature_drift);
    assert_external_probe_path_rejected(
        &fixture,
        &signature_drift_path,
        &signed.trust_path,
        "payload_sha256 mismatch",
    );

    let mut unknown_field = signed.receipt;
    unknown_field["fixture_only_bypass"] = Value::Bool(true);
    let unknown_field_path = fixture.root.join("external-receipt-unknown-field.json");
    write_json(&unknown_field_path, &unknown_field);
    assert_external_probe_path_rejected(
        &fixture,
        &unknown_field_path,
        &signed.trust_path,
        "unknown field",
    );
}

#[test]
fn inspect_seed_fleet_cli_rejects_capacity_lane_and_advertising_drift() {
    let Some(fixture) = build_fleet_fixture("reject-drift") else {
        return;
    };
    let original = std::fs::read_to_string(&fixture.config).expect("read fleet config");
    let cases = [
        (
            "capacity",
            original.replace("\"target_model_count\": 20", "\"target_model_count\": 21"),
            "target_model_count must be 20",
        ),
        (
            "lane-count",
            original.replace("\"replica_lane_count\": 2", "\"replica_lane_count\": 1"),
            "replica_lane_count must be 2",
        ),
        (
            "advertise-before-probe",
            original.replace(
                "\"advertise_only_after_external_probe\": true",
                "\"advertise_only_after_external_probe\": false",
            ),
            "advertise_only_after_external_probe must be true",
        ),
        (
            "project-model-bytes",
            original.replace(
                "\"project_server_serves_model_bytes\": false",
                "\"project_server_serves_model_bytes\": true",
            ),
            "project_server_serves_model_bytes must be false",
        ),
    ];

    for (label, drifted, expected) in cases {
        let path = fixture.root.join(format!("{label}.json"));
        std::fs::write(&path, drifted).expect("write drifted fleet config");
        let output = Command::new(minimodel_bin())
            .args(["inspect-seed-fleet", &path.to_string_lossy()])
            .output()
            .expect("run inspect-seed-fleet drift");
        assert!(
            !output.status.success(),
            "{label}: {}",
            output_text(&output)
        );
        assert!(
            output_text(&output).contains(expected),
            "{label}: {}",
            output_text(&output)
        );
    }
}

#[test]
fn inspect_seed_fleet_cli_rejects_one_artifact_aliased_as_multiple_models() {
    let Some(fixture) = build_fleet_fixture("reject-artifact-alias") else {
        return;
    };
    let artifact = sibling_q8_fixture().expect("fixture remains available");
    let alias_model_id = "tinyrustlm/seed-fleet-alias-q8";
    let alias_share = fixture.root.join("share-alias");
    let output = Command::new(minimodel_bin())
        .args([
            "prepare-peer-share",
            &artifact.to_string_lossy(),
            &alias_share.to_string_lossy(),
            alias_model_id,
            "TinyRustLM/Fixture-Q8-Alias",
        ])
        .output()
        .expect("prepare aliased peer share");
    assert!(output.status.success(), "{}", output_text(&output));

    let alias_manifest = alias_share.join("tiny-test-model-q8.minimodel.manifest.v0");
    let alias_piece_dir = alias_share.join("tiny-test-model-q8.peer-pieces");
    let alias_a_port = unused_port();
    let alias_b_port = unused_port();
    let alias_a_url = format!("http://127.0.0.1:{alias_a_port}/models/alias/");
    let alias_b_url = format!("http://127.0.0.1:{alias_b_port}/models/alias/");
    let alias_a_state = fixture.root.join("seed-a-alias.state.json");
    let alias_b_state = fixture.root.join("seed-b-alias.state.json");
    write_state(
        &alias_manifest,
        &alias_piece_dir,
        &alias_a_state,
        &alias_a_url,
    );
    write_state(
        &alias_manifest,
        &alias_piece_dir,
        &alias_b_state,
        &alias_b_url,
    );

    let mut config: Value =
        serde_json::from_str(&std::fs::read_to_string(&fixture.config).expect("read fleet config"))
            .expect("parse fleet config");
    config["models"]
        .as_array_mut()
        .expect("models array")
        .push(serde_json::json!({
            "model_id": alias_model_id,
            "enabled": true,
            "manifest_path": "share-alias/tiny-test-model-q8.minimodel.manifest.v0",
            "piece_dir": "share-alias/tiny-test-model-q8.peer-pieces"
        }));
    let lanes = config["lanes"].as_array_mut().expect("lanes array");
    lanes[0]["endpoints"]
        .as_array_mut()
        .expect("lane A endpoints")
        .push(serde_json::json!({
            "model_id": alias_model_id,
            "state_path": "seed-a-alias.state.json",
            "bind_addr": format!("127.0.0.1:{alias_a_port}"),
            "public_base_url": alias_a_url
        }));
    lanes[1]["endpoints"]
        .as_array_mut()
        .expect("lane B endpoints")
        .push(serde_json::json!({
            "model_id": alias_model_id,
            "state_path": "seed-b-alias.state.json",
            "bind_addr": format!("127.0.0.1:{alias_b_port}"),
            "public_base_url": alias_b_url
        }));
    let alias_config = fixture.root.join("artifact-alias.json");
    std::fs::write(
        &alias_config,
        serde_json::to_vec_pretty(&config).expect("serialize aliased fleet"),
    )
    .expect("write aliased fleet");

    let output = Command::new(minimodel_bin())
        .args(["inspect-seed-fleet", &alias_config.to_string_lossy()])
        .output()
        .expect("inspect aliased fleet");
    assert!(!output.status.success(), "{}", output_text(&output));
    assert!(
        output_text(&output).contains("duplicate artifact_sha256"),
        "{}",
        output_text(&output)
    );
}

#[test]
fn inspect_seed_fleet_cli_accepts_twenty_distinct_fixture_artifacts() {
    let Some(fixture) = build_distinct_capacity_fixture() else {
        return;
    };

    let output = Command::new(minimodel_bin())
        .args(["inspect-seed-fleet", &fixture.config.to_string_lossy()])
        .output()
        .expect("inspect full-capacity fleet");
    assert!(output.status.success(), "{}", output_text(&output));
    let text = output_text(&output);
    assert!(text.contains("active_model_count: 20"), "{text}");
    assert!(text.contains("remaining_model_capacity: 0"), "{text}");
    assert!(text.contains("peer_endpoint_count: 40"), "{text}");
}

#[test]
fn serve_seed_lane_cli_serves_verified_metadata_and_pieces_from_either_lane() {
    let Some(fixture) = build_fleet_fixture("serve") else {
        return;
    };

    for (lane_id, base_url) in [
        ("seed-a", fixture.lane_a_url.as_str()),
        ("seed-b", fixture.lane_b_url.as_str()),
    ] {
        let mut process = SeedLaneProcess::start(&fixture.config, lane_id);

        let status = wait_for_http(
            base_url,
            "minimodel.peer_status.v0.json",
            process.child_mut(),
        );
        assert!(status.contains("200 OK"), "{lane_id}: {status}");
        assert!(status.contains(MODEL_ID), "{lane_id}: {status}");
        assert!(
            status.contains("\"byte_count\": 8832"),
            "{lane_id}: {status}"
        );

        let piece_set =
            http_get(base_url, "minimodel.peer_piece_set.v0").expect("fetch peer piece set");
        assert!(piece_set.contains("200 OK"), "{lane_id}: {piece_set}");
        assert!(
            piece_set.contains("chunks.count=1"),
            "{lane_id}: {piece_set}"
        );

        process.stop();
    }
}

#[test]
fn serve_seed_lane_cli_releases_all_prebound_ports_when_a_later_bind_collides() {
    let Some(fixture) = build_distinct_fixture(2) else {
        return;
    };
    let config: Value = serde_json::from_str(
        &std::fs::read_to_string(&fixture.config).expect("read two-model fleet"),
    )
    .expect("parse two-model fleet");
    let endpoints = config["lanes"][0]["endpoints"]
        .as_array()
        .expect("lane A endpoints");
    let first_bind = endpoints[0]["bind_addr"]
        .as_str()
        .expect("first bind address")
        .to_string();
    let second_bind = endpoints[1]["bind_addr"]
        .as_str()
        .expect("second bind address")
        .to_string();
    let collision = TcpListener::bind(&second_bind).expect("hold later endpoint port");

    let output = Command::new(minimodel_bin())
        .args([
            "serve-seed-lane",
            &fixture.config.to_string_lossy(),
            "seed-a",
        ])
        .output()
        .expect("run colliding seed lane");
    assert!(!output.status.success(), "{}", output_text(&output));
    assert!(
        output_text(&output).contains("failed to bind seed lane seed-a"),
        "{}",
        output_text(&output)
    );
    drop(collision);

    let released = TcpListener::bind(&first_bind)
        .unwrap_or_else(|error| panic!("first prebound endpoint remained open: {error}"));
    drop(released);
}

#[test]
fn independent_seed_lane_process_survives_other_lane_shutdown() {
    let Some(fixture) = build_fleet_fixture("lane-independence") else {
        return;
    };
    let mut lane_a = SeedLaneProcess::start(&fixture.config, "seed-a");
    let mut lane_b = SeedLaneProcess::start(&fixture.config, "seed-b");
    let lane_a_status = wait_for_http(
        &fixture.lane_a_url,
        "minimodel.peer_status.v0.json",
        lane_a.child_mut(),
    );
    let lane_b_status = wait_for_http(
        &fixture.lane_b_url,
        "minimodel.peer_status.v0.json",
        lane_b.child_mut(),
    );
    assert!(lane_a_status.contains("200 OK"), "{lane_a_status}");
    assert!(lane_b_status.contains("200 OK"), "{lane_b_status}");

    lane_a.stop();
    let still_serving = http_get(&fixture.lane_b_url, "minimodel.peer_status.v0.json")
        .expect("lane B remains reachable");
    assert!(still_serving.contains("200 OK"), "{still_serving}");
    lane_b.stop();
}

#[test]
fn owned_shutdown_file_stops_only_its_lane_gracefully() {
    let Some(fixture) = build_fleet_fixture("graceful-shutdown") else {
        return;
    };
    let lane_a_shutdown = fixture.root.join("seed-a.shutdown");
    let lane_b_shutdown = fixture.root.join("seed-b.shutdown");
    let mut lane_a =
        SeedLaneProcess::start_with_shutdown_file(&fixture.config, "seed-a", &lane_a_shutdown);
    let mut lane_b =
        SeedLaneProcess::start_with_shutdown_file(&fixture.config, "seed-b", &lane_b_shutdown);
    let lane_a_status = wait_for_http(
        &fixture.lane_a_url,
        "minimodel.peer_status.v0.json",
        lane_a.child_mut(),
    );
    let lane_b_status = wait_for_http(
        &fixture.lane_b_url,
        "minimodel.peer_status.v0.json",
        lane_b.child_mut(),
    );
    assert!(lane_a_status.contains("200 OK"), "{lane_a_status}");
    assert!(lane_b_status.contains("200 OK"), "{lane_b_status}");

    std::fs::write(&lane_a_shutdown, b"stop\n").expect("signal lane A shutdown");
    let deadline = Instant::now() + Duration::from_secs(5);
    let lane_a_status = loop {
        if let Some(status) = lane_a.child_mut().try_wait().expect("poll lane A") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "lane A did not honor its shutdown file"
        );
        thread::sleep(Duration::from_millis(25));
    };
    assert!(lane_a_status.success(), "lane A should stop gracefully");

    let still_serving = http_get(&fixture.lane_b_url, "minimodel.peer_status.v0.json")
        .expect("lane B remains reachable after lane A graceful stop");
    assert!(still_serving.contains("200 OK"), "{still_serving}");

    std::fs::write(&lane_b_shutdown, b"stop\n").expect("signal lane B shutdown");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = lane_b.child_mut().try_wait().expect("poll lane B") {
            assert!(status.success(), "lane B should stop gracefully");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "lane B did not honor its shutdown file"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn seed_lane_rejects_shutdown_control_outside_the_fleet_root() {
    let Some(fixture) = build_fleet_fixture("reject-external-shutdown") else {
        return;
    };
    let outside_path = fixture
        .root
        .parent()
        .expect("fleet root has parent")
        .join(format!("outside-shutdown-{}.signal", std::process::id()));
    let output = Command::new(minimodel_bin())
        .args([
            "serve-seed-lane",
            &fixture.config.to_string_lossy(),
            "seed-a",
            &outside_path.to_string_lossy(),
        ])
        .output()
        .expect("reject outside shutdown control");
    assert!(!output.status.success(), "{}", output_text(&output));
    assert!(
        output_text(&output).contains("shutdown path must stay inside the fleet root"),
        "{}",
        output_text(&output)
    );
}

struct SignedExternalProbeFixture {
    receipt_path: PathBuf,
    trust_path: PathBuf,
    receipt: Value,
}

fn write_external_probe_fixture(
    fixture: &FleetFixture,
    label: &str,
    checked_offset_seconds: i64,
    expires_offset_seconds: i64,
    network_scope: &str,
    revoked: bool,
) -> SignedExternalProbeFixture {
    let piece_dir = fixture.root.join("share/tiny-test-model-q8.peer-pieces");
    let piece_set_path = piece_dir.join("minimodel.peer_piece_set.v0");
    let piece_set_bytes = std::fs::read(&piece_set_path).expect("read fixture piece set");
    let piece_set_text = std::str::from_utf8(&piece_set_bytes).expect("piece set is UTF-8");
    let first_piece_name = piece_set_field(piece_set_text, "piece.0.file");
    let first_piece_bytes =
        std::fs::read(piece_dir.join(&first_piece_name)).expect("read first fixture piece");
    let state_bytes =
        std::fs::read(fixture.root.join("seed-a.state.json")).expect("read lane state");
    let status_bytes = br#"{"status":"fixture-external-observer-reached-peer"}"#;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock follows Unix epoch")
        .as_secs() as i64;
    let signing_key = SigningKey::from_bytes(&[0x5a; 32]);
    let public_key = signing_key.verifying_key().to_bytes();

    let mut receipt = serde_json::json!({
        "receipt_kind": "minimodel.external_reachability_receipt",
        "receipt_version": 0,
        "receipt_id": format!("fixture-receipt-{label}"),
        "observer_id": "fixture-external-observer",
        "observer_key_id": "fixture-observer-key-v0",
        "fleet_id": "test-fleet-external-receipt",
        "lane_id": "seed-a",
        "model_id": MODEL_ID,
        "public_base_url": fixture.lane_a_url,
        "artifact_sha256": piece_set_field(piece_set_text, "artifact.sha256"),
        "piece_set_sha256": sha256_prefixed(&piece_set_bytes),
        "checked_utc": format_test_utc(now + checked_offset_seconds),
        "expires_utc": format_test_utc(now + expires_offset_seconds),
        "probe_implementation_version": "fixture-observer-v0",
        "network_scope": network_scope,
        "checked_routes": [
            {
                "route": "minimodel.peer_status.v0.json",
                "http_status": 200,
                "body_byte_count": status_bytes.len(),
                "body_sha256": sha256_prefixed(status_bytes)
            },
            {
                "route": "minimodel.peer_companion_state.v0.json",
                "http_status": 200,
                "body_byte_count": state_bytes.len(),
                "body_sha256": sha256_prefixed(&state_bytes)
            },
            {
                "route": "minimodel.peer_piece_set.v0",
                "http_status": 200,
                "body_byte_count": piece_set_bytes.len(),
                "body_sha256": sha256_prefixed(&piece_set_bytes)
            },
            {
                "route": first_piece_name,
                "http_status": 200,
                "body_byte_count": first_piece_bytes.len(),
                "body_sha256": sha256_prefixed(&first_piece_bytes)
            }
        ],
        "signature": {
            "kind": "ed25519",
            "payload_sha256": "",
            "value": ""
        }
    });
    let payload = external_probe_signature_payload(&receipt);
    receipt["signature"]["payload_sha256"] = Value::String(sha256_prefixed(&payload));
    receipt["signature"]["value"] = Value::String(format!(
        "ed25519-base64:{}",
        BASE64_STANDARD.encode(signing_key.sign(&payload).to_bytes())
    ));

    let trust = serde_json::json!({
        "trust_kind": "minimodel.external_probe_trust",
        "trust_version": 0,
        "keys": [{
            "key_id": "fixture-observer-key-v0",
            "observer_id": "fixture-external-observer",
            "algorithm": "ed25519",
            "usage": "minimodel-external-reachability-v0",
            "observer_scope": "external-independent-observer",
            "public_key_base64": BASE64_STANDARD.encode(public_key),
            "public_key_sha256": sha256_prefixed(&public_key),
            "revoked": revoked
        }]
    });
    let receipt_path = fixture.root.join(format!("external-receipt-{label}.json"));
    let trust_path = fixture.root.join(format!("external-trust-{label}.json"));
    write_json(&receipt_path, &receipt);
    write_json(&trust_path, &trust);
    SignedExternalProbeFixture {
        receipt_path,
        trust_path,
        receipt,
    }
}

fn inspect_external_probe(
    fixture: &FleetFixture,
    receipt_path: &Path,
    trust_path: &Path,
) -> std::process::Output {
    Command::new(minimodel_bin())
        .args([
            "inspect-external-probe-receipt",
            &receipt_path.to_string_lossy(),
            &trust_path.to_string_lossy(),
            &fixture.config.to_string_lossy(),
        ])
        .output()
        .expect("inspect external probe fixture")
}

fn assert_external_probe_rejected(
    fixture: &FleetFixture,
    signed: &SignedExternalProbeFixture,
    expected: &str,
) {
    assert_external_probe_path_rejected(
        fixture,
        &signed.receipt_path,
        &signed.trust_path,
        expected,
    );
}

fn assert_external_probe_path_rejected(
    fixture: &FleetFixture,
    receipt_path: &Path,
    trust_path: &Path,
    expected: &str,
) {
    let output = inspect_external_probe(fixture, receipt_path, trust_path);
    assert!(!output.status.success(), "{}", output_text(&output));
    assert!(
        output_text(&output).contains(expected),
        "expected {expected:?} in {}",
        output_text(&output)
    );
}

fn external_probe_signature_payload(receipt: &Value) -> Vec<u8> {
    let mut fields = BTreeMap::<String, String>::new();
    for key in [
        "receipt_kind",
        "receipt_id",
        "observer_id",
        "observer_key_id",
        "fleet_id",
        "lane_id",
        "model_id",
        "public_base_url",
        "artifact_sha256",
        "piece_set_sha256",
        "checked_utc",
        "expires_utc",
        "probe_implementation_version",
        "network_scope",
    ] {
        fields.insert(key.to_string(), json_string(receipt, key));
    }
    fields.insert(
        "receipt_version".to_string(),
        receipt["receipt_version"]
            .as_u64()
            .expect("receipt_version is u64")
            .to_string(),
    );
    let routes = receipt["checked_routes"]
        .as_array()
        .expect("checked_routes is an array");
    fields.insert("checked_routes.count".to_string(), routes.len().to_string());
    for (index, route) in routes.iter().enumerate() {
        fields.insert(
            format!("checked_routes.{index}.route"),
            json_string(route, "route"),
        );
        fields.insert(
            format!("checked_routes.{index}.http_status"),
            route["http_status"]
                .as_u64()
                .expect("http_status is u64")
                .to_string(),
        );
        fields.insert(
            format!("checked_routes.{index}.body_byte_count"),
            route["body_byte_count"]
                .as_u64()
                .expect("body_byte_count is u64")
                .to_string(),
        );
        fields.insert(
            format!("checked_routes.{index}.body_sha256"),
            json_string(route, "body_sha256"),
        );
    }
    serde_json::to_vec(&fields).expect("serialize canonical external probe payload")
}

fn json_string(value: &Value, key: &str) -> String {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} is a string"))
        .to_string()
}

fn piece_set_field(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|line| {
            let (observed_key, value) = line.split_once('=')?;
            (observed_key == key).then(|| value.to_string())
        })
        .unwrap_or_else(|| panic!("piece set contains {key}"))
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(value).expect("serialize test JSON"),
    )
    .expect("write test JSON");
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn format_test_utc(seconds: i64) -> String {
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
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    (year + i64::from(month <= 2), month, day)
}

fn build_fleet_fixture(label: &str) -> Option<FleetFixture> {
    let Some(artifact) = sibling_q8_fixture() else {
        eprintln!("skipping seed fleet CLI test; TinyRustLM fixture not present");
        return None;
    };
    let root = unique_dir(&format!("seed-fleet-{label}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create fleet fixture root");

    let share = root.join("share");
    let output = Command::new(minimodel_bin())
        .args([
            "prepare-peer-share",
            &artifact.to_string_lossy(),
            &share.to_string_lossy(),
            MODEL_ID,
            "TinyRustLM/Fixture-Q8",
        ])
        .output()
        .expect("prepare peer share");
    assert!(output.status.success(), "{}", output_text(&output));

    let manifest = share.join("tiny-test-model-q8.minimodel.manifest.v0");
    let piece_dir = share.join("tiny-test-model-q8.peer-pieces");
    let lane_a_port = unused_port();
    let lane_b_port = unused_port();
    assert_ne!(lane_a_port, lane_b_port);
    let lane_a_url = format!("http://127.0.0.1:{lane_a_port}/models/test/");
    let lane_b_url = format!("http://127.0.0.1:{lane_b_port}/models/test/");
    let lane_a_state = root.join("seed-a.state.json");
    let lane_b_state = root.join("seed-b.state.json");
    write_state(&manifest, &piece_dir, &lane_a_state, &lane_a_url);
    write_state(&manifest, &piece_dir, &lane_b_state, &lane_b_url);

    let config = root.join("seed-fleet.v0.json");
    let text = format!(
        concat!(
            "{{\n",
            "  \"fleet_kind\": \"minimodel.seed_fleet\",\n",
            "  \"fleet_id\": \"test-fleet-{label}\",\n",
            "  \"fleet_version\": 0,\n",
            "  \"target_model_count\": 20,\n",
            "  \"replica_lane_count\": 2,\n",
            "  \"models\": [{{\"model_id\": \"{model_id}\", ",
            "\"enabled\": true, ",
            "\"manifest_path\": \"share/tiny-test-model-q8.minimodel.manifest.v0\", ",
            "\"piece_dir\": \"share/tiny-test-model-q8.peer-pieces\"}}],\n",
            "  \"lanes\": [\n",
            "    {{\"lane_id\": \"seed-a\", \"endpoints\": [",
            "{{\"model_id\": \"{model_id}\", \"state_path\": \"seed-a.state.json\", ",
            "\"bind_addr\": \"127.0.0.1:{lane_a_port}\", \"public_base_url\": \"{lane_a_url}\"}}]}},\n",
            "    {{\"lane_id\": \"seed-b\", \"endpoints\": [",
            "{{\"model_id\": \"{model_id}\", \"state_path\": \"seed-b.state.json\", ",
            "\"bind_addr\": \"127.0.0.1:{lane_b_port}\", \"public_base_url\": \"{lane_b_url}\"}}]}}\n",
            "  ],\n",
            "  \"boundary\": {{\n",
            "    \"operator_consent\": true,\n",
            "    \"advertise_only_after_external_probe\": true,\n",
            "    \"project_server_serves_model_bytes\": false,\n",
            "    \"project_server_runs_model_bytes\": false,\n",
            "    \"project_server_proxies_model_bytes\": false\n",
            "  }}\n",
            "}}\n"
        ),
        model_id = MODEL_ID,
        label = label,
        lane_a_port = lane_a_port,
        lane_a_url = lane_a_url,
        lane_b_port = lane_b_port,
        lane_b_url = lane_b_url,
    );
    std::fs::write(&config, text).expect("write seed fleet config");

    Some(FleetFixture {
        root,
        config,
        lane_a_url,
        lane_b_url,
    })
}

fn build_distinct_capacity_fixture() -> Option<FleetFixture> {
    build_distinct_fixture(20)
}

fn build_distinct_fixture(model_count: usize) -> Option<FleetFixture> {
    assert!((1..=20).contains(&model_count));
    let source_artifact = sibling_q8_fixture()?;
    let source_bytes = std::fs::read(source_artifact).expect("read q8 fixture");
    let root = unique_dir("seed-fleet-capacity-20");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("artifacts")).expect("create capacity artifacts");
    let ports = reserve_unique_ports(model_count * 2);
    let mut models = Vec::new();
    let mut lane_a_endpoints = Vec::new();
    let mut lane_b_endpoints = Vec::new();
    let mut first_lane_a_url = String::new();
    let mut first_lane_b_url = String::new();

    for index in 0..model_count {
        let model_id = format!("tinyrustlm/seed-capacity-{index:02}-q8");
        let artifact_name = format!("capacity-{index:02}.slm");
        let artifact_path = root.join("artifacts").join(&artifact_name);
        std::fs::write(
            &artifact_path,
            distinct_q8_fixture(&source_bytes, index as u8),
        )
        .expect("write distinct q8 fixture");
        let share_name = format!("share-{index:02}");
        let share_dir = root.join(&share_name);
        let output = Command::new(minimodel_bin())
            .args([
                "prepare-peer-share",
                &artifact_path.to_string_lossy(),
                &share_dir.to_string_lossy(),
                &model_id,
                &format!("TinyRustLM/Capacity-Fixture-{index:02}"),
            ])
            .output()
            .expect("prepare capacity peer share");
        assert!(output.status.success(), "{}", output_text(&output));

        let stem = format!("capacity-{index:02}");
        let manifest_name = format!("{stem}.minimodel.manifest.v0");
        let piece_dir_name = format!("{stem}.peer-pieces");
        let manifest_path = share_dir.join(&manifest_name);
        let piece_dir = share_dir.join(&piece_dir_name);
        let lane_a_port = ports[index * 2];
        let lane_b_port = ports[index * 2 + 1];
        let lane_a_url = format!("http://127.0.0.1:{lane_a_port}/models/{index:02}/");
        let lane_b_url = format!("http://127.0.0.1:{lane_b_port}/models/{index:02}/");
        if index == 0 {
            first_lane_a_url.clone_from(&lane_a_url);
            first_lane_b_url.clone_from(&lane_b_url);
        }
        let lane_a_state_name = format!("seed-a-{index:02}.state.json");
        let lane_b_state_name = format!("seed-b-{index:02}.state.json");
        write_state(
            &manifest_path,
            &piece_dir,
            &root.join(&lane_a_state_name),
            &lane_a_url,
        );
        write_state(
            &manifest_path,
            &piece_dir,
            &root.join(&lane_b_state_name),
            &lane_b_url,
        );

        models.push(serde_json::json!({
            "model_id": model_id,
            "enabled": true,
            "manifest_path": format!("{share_name}/{manifest_name}"),
            "piece_dir": format!("{share_name}/{piece_dir_name}")
        }));
        lane_a_endpoints.push(serde_json::json!({
            "model_id": model_id,
            "state_path": lane_a_state_name,
            "bind_addr": format!("127.0.0.1:{lane_a_port}"),
            "public_base_url": lane_a_url
        }));
        lane_b_endpoints.push(serde_json::json!({
            "model_id": model_id,
            "state_path": lane_b_state_name,
            "bind_addr": format!("127.0.0.1:{lane_b_port}"),
            "public_base_url": lane_b_url
        }));
    }

    let config_value = serde_json::json!({
        "fleet_kind": "minimodel.seed_fleet",
        "fleet_id": format!("test-fleet-capacity-{model_count}"),
        "fleet_version": 0,
        "target_model_count": 20,
        "replica_lane_count": 2,
        "models": models,
        "lanes": [
            {"lane_id": "seed-a", "endpoints": lane_a_endpoints},
            {"lane_id": "seed-b", "endpoints": lane_b_endpoints}
        ],
        "boundary": {
            "operator_consent": true,
            "advertise_only_after_external_probe": true,
            "project_server_serves_model_bytes": false,
            "project_server_runs_model_bytes": false,
            "project_server_proxies_model_bytes": false
        }
    });
    let config = root.join("seed-fleet.v0.json");
    std::fs::write(
        &config,
        serde_json::to_vec_pretty(&config_value).expect("serialize capacity fleet"),
    )
    .expect("write capacity fleet");

    Some(FleetFixture {
        root,
        config,
        lane_a_url: first_lane_a_url,
        lane_b_url: first_lane_b_url,
    })
}

fn distinct_q8_fixture(source: &[u8], variant: u8) -> Vec<u8> {
    let mut bytes = source.to_vec();
    if variant != 0 {
        let directory_offset = u64::from_le_bytes(
            bytes[80..88]
                .try_into()
                .expect("q8 tensor directory offset"),
        ) as usize;
        let payload_offset = u64::from_le_bytes(
            bytes[directory_offset + 32..directory_offset + 40]
                .try_into()
                .expect("q8 tensor payload offset"),
        ) as usize;
        bytes[payload_offset] ^= variant;
        let checksum = slm_checksum(&bytes);
        bytes[100..108].copy_from_slice(&checksum.to_le_bytes());
    }
    bytes
}

fn slm_checksum(bytes: &[u8]) -> u64 {
    let mut checksum = 0x9e3779b97f4a7c15u64;
    for (index, byte) in bytes.iter().enumerate() {
        let value = if (100..108).contains(&index) {
            0
        } else {
            *byte
        };
        checksum ^= u64::from(value).wrapping_add((index as u64) << 1);
        checksum = checksum.rotate_left(7).wrapping_mul(0x100000001b3);
    }
    checksum
}

fn reserve_unique_ports(count: usize) -> Vec<u16> {
    let mut listeners = Vec::with_capacity(count);
    for _ in 0..count {
        listeners.push(TcpListener::bind("127.0.0.1:0").expect("reserve unique port"));
    }
    listeners
        .iter()
        .map(|listener| listener.local_addr().expect("read reserved port").port())
        .collect()
}

fn write_state(manifest: &Path, piece_dir: &Path, state: &Path, peer_url: &str) {
    let output = Command::new(minimodel_bin())
        .args([
            "write-peer-companion-state",
            &manifest.to_string_lossy(),
            &piece_dir.to_string_lossy(),
            &state.to_string_lossy(),
            peer_url,
        ])
        .output()
        .expect("write peer companion state");
    assert!(output.status.success(), "{}", output_text(&output));
}

fn unused_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral test port")
        .local_addr()
        .expect("read ephemeral test port")
        .port()
}

fn wait_for_http(base_url: &str, route: &str, child: &mut Child) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().expect("inspect seed lane process") {
            let mut stderr = String::new();
            if let Some(mut stream) = child.stderr.take() {
                let _ = stream.read_to_string(&mut stderr);
            }
            panic!("seed lane exited before serving with {status}: {stderr}");
        }
        if let Ok(response) = http_get(base_url, route) {
            return response;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {base_url}{route}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn http_get(base_url: &str, route: &str) -> std::io::Result<String> {
    let without_scheme = base_url
        .strip_prefix("http://")
        .expect("test peer URL must use HTTP");
    let (authority, path) = without_scheme
        .split_once('/')
        .expect("test peer URL must include a path");
    let mut stream = TcpStream::connect(authority)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "GET /{path}{route} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}
