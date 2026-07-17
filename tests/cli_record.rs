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
