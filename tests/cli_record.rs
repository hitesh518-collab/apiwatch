use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;
use predicates::prelude::*;

fn temp_lock_path(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "apiwatch-{name}-{}-{suffix}.lock",
        std::process::id()
    ))
}

#[test]
fn record_creates_a_value_free_v2_observed_lock() {
    let output = temp_lock_path("observed-record");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-json",
            "testdata/observed/portfolio-empty.json",
            "--name",
            "portfolio",
            "--output",
            output_arg,
        ])
        .assert()
        .success();

    let lock = fs::read_to_string(&output).expect("recorded lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.starts_with("version: 2\n"));
    assert!(lock.contains("provenance: observed"));
    assert!(!lock.contains("recording-secret-001"));
}

#[test]
fn record_repeatable_map_at_writes_value_free_maps() {
    let output = temp_lock_path("observed-map");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-json",
            "testdata/observed/portfolio-map-initial.json",
            "--name",
            "portfolio",
            "--output",
            output_arg,
            "--map-at",
            "$.by_broker",
            "--map-at",
            "$.state.by_region",
        ])
        .assert()
        .success();

    let lock = fs::read_to_string(&output).expect("recorded lock should exist");
    fs::remove_file(&output).ok();

    assert_eq!(lock.matches("kind: map").count(), 2);
    assert!(!lock.contains("acme"));
    assert!(!lock.contains("globex"));
    assert!(!lock.contains("map-secret-initial"));
}

#[test]
fn merge_into_recorded_map_needs_no_repeated_annotation() {
    let output = temp_lock_path("observed-map-merge");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-json",
            "testdata/observed/portfolio-map-initial.json",
            "--name",
            "portfolio",
            "--output",
            output_arg,
            "--map-at",
            "$.by_broker",
        ])
        .assert()
        .success();
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-json",
            "testdata/observed/portfolio-map-merged.json",
            "--name",
            "portfolio",
            "--output",
            output_arg,
            "--merge",
        ])
        .assert()
        .success();

    let lock = fs::read_to_string(&output).expect("merged lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.contains("kind: map"));
    assert!(!lock.contains("initech"));
    assert!(!lock.contains("map-secret-merged"));
}

#[test]
fn invalid_map_annotations_fail_without_creating_a_lock() {
    for annotation in [
        "$.by-broker",
        "$.by_broker[0]",
        "$..by_broker",
        "$.missing",
        "$.state.by_region.in.active",
    ] {
        let output = temp_lock_path("observed-map-invalid");
        let output_arg = output.to_str().expect("temp path should be valid UTF-8");

        Command::cargo_bin("apiwatch")
            .expect("binary should build")
            .args([
                "record",
                "--from-json",
                "testdata/observed/portfolio-map-initial.json",
                "--name",
                "portfolio",
                "--output",
                output_arg,
                "--map-at",
                annotation,
            ])
            .assert()
            .code(2)
            .stdout(predicate::str::is_empty())
            .stderr(predicate::str::contains("map annotation"));

        assert!(!output.exists(), "{annotation} should not create a lock");
    }
}

#[test]
fn duplicate_map_annotation_does_not_overwrite_a_recorded_lock() {
    let output = temp_lock_path("observed-map-duplicate");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-json",
            "testdata/observed/portfolio-empty.json",
            "--name",
            "portfolio",
            "--output",
            output_arg,
        ])
        .assert()
        .success();
    let before = fs::read_to_string(&output).expect("initial lock should exist");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-json",
            "testdata/observed/portfolio-map-initial.json",
            "--name",
            "portfolio",
            "--output",
            output_arg,
            "--merge",
            "--map-at",
            "$.by_broker",
            "--map-at",
            "$.by_broker",
        ])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("duplicate map annotation"));

    let after = fs::read_to_string(&output).expect("initial lock should remain");
    fs::remove_file(&output).ok();
    assert_eq!(after, before);
}

#[test]
fn record_from_har_single_entry_creates_observed_lock() {
    let output = temp_lock_path("har-single");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/single-entry.har",
            "--output",
            output_arg,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 1 endpoints:"))
        .stdout(predicate::str::contains("GET /users/42"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.starts_with("version: 2\n"));
    assert!(lock.contains("provenance: observed"));
    assert!(lock.contains("GET /users/42"));
    assert!(!lock.contains("\"alice\""));
}

#[test]
fn record_from_har_multi_entry_groups_by_path() {
    let output = temp_lock_path("har-multi");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/multi-entry.har",
            "--output",
            output_arg,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 3 endpoints:"))
        .stdout(predicate::str::contains("GET /users/42"))
        .stdout(predicate::str::contains("GET /users/99"))
        .stdout(predicate::str::contains("GET /orders/7"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.starts_with("version: 2\n"));
    assert!(lock.contains("provenance: observed"));
}

#[test]
fn record_from_har_with_path_identity_groups_entries() {
    let output = temp_lock_path("har-identity");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/multi-entry.har",
            "--output",
            output_arg,
            "--path-identity",
            "GET /users",
            "--path-identity",
            "GET /orders",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 2 endpoints:"))
        .stdout(predicate::str::contains("GET /orders: 1 sample(s)"))
        .stdout(predicate::str::contains("GET /users: 2 sample(s)"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.contains("GET /users"));
    assert!(lock.contains("GET /orders"));
}

#[test]
fn record_from_har_reports_skipped_entries() {
    let output = temp_lock_path("har-mixed");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/mixed-content.har",
            "--output",
            output_arg,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 1 endpoints:"))
        .stdout(predicate::str::contains("Skipped 3 response(s):"))
        .stdout(predicate::str::contains("non-JSON content type"))
        .stdout(predicate::str::contains("JSON parse error"))
        .stdout(predicate::str::contains("empty body"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.starts_with("version: 2\n"));
}

#[test]
fn record_from_har_no_json_entries_fails() {
    let output = temp_lock_path("har-no-json");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/non-json-entry.har",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no HAR entries matched"));

    assert!(!output.exists());
}

#[test]
fn record_from_har_with_status_filter() {
    let output = temp_lock_path("har-status");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/single-entry.har",
            "--output",
            output_arg,
            "--status",
            "200",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 1 endpoints:"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();
    assert!(lock.starts_with("version: 2\n"));
}

#[test]
fn record_from_har_with_name_merges_all_under_single_key() {
    let output = temp_lock_path("har-name");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/multi-entry.har",
            "--output",
            output_arg,
            "--name",
            "my-api",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 3 endpoints:"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.contains("my-api"));
    assert_eq!(lock.matches("provenance: observed").count(), 1);
}

#[test]
fn record_from_har_file_not_found() {
    let output = temp_lock_path("har-missing");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/does-not-exist.har",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("failed to read HAR file"));
}

#[test]
fn record_from_har_mutual_exclusion_with_from_json() {
    let output = temp_lock_path("har-exclusive");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/single-entry.har",
            "--from-json",
            "testdata/observed/portfolio-empty.json",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn record_from_har_existing_from_json_still_works() {
    let output = temp_lock_path("har-backcompat");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-json",
            "testdata/observed/portfolio-empty.json",
            "--name",
            "portfolio",
            "--output",
            output_arg,
        ])
        .assert()
        .success();

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.starts_with("version: 2\n"));
    assert!(lock.contains("provenance: observed"));
}
