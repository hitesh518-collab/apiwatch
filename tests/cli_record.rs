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
        .stderr(predicate::str::contains("only one source may be specified"));
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

#[test]
fn record_from_har_base64_encoded_is_skipped() {
    let output = temp_lock_path("har-base64");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/base64-entry.har",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no HAR entries matched"));

    assert!(!output.exists());
}

#[test]
fn record_from_har_merge_with_same_path_identity() {
    let output = temp_lock_path("har-merge");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/single-entry.har",
            "--output",
            output_arg,
            "--path-identity",
            "GET /users",
        ])
        .assert()
        .success();

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
            "--merge",
        ])
        .assert()
        .success();

    let lock = fs::read_to_string(&output).expect("merged lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.starts_with("version: 2\n"));
    assert!(lock.contains("GET /users"));
    assert_eq!(lock.matches("provenance: observed").count(), 1);
}

#[test]
fn record_from_har_with_map_at_applies_map_annotations() {
    let output = temp_lock_path("har-map-at");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/single-entry.har",
            "--output",
            output_arg,
            "--name",
            "map-api",
            "--map-at",
            "$",
        ])
        .assert()
        .success();

    let lock = fs::read_to_string(&output).expect("should exist");
    fs::remove_file(&output).ok();

    assert!(lock.starts_with("version: 2\n"));
    assert!(lock.contains("kind: map"));
    assert!(!lock.contains("\"alice\""));
}

#[test]
fn record_from_url_creates_observed_entry() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
    let address = listener.local_addr().expect("listener should have address");
    let url = format!("http://{}/users", address);

    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("should accept");
        let mut buf = Vec::new();
        while !buf.ends_with(b"\r\n\r\n") {
            let mut byte = [0u8; 1];
            stream.read_exact(&mut byte).expect("should read headers");
            buf.push(byte[0]);
        }
        let body = r#"{"id":1,"name":"alice"}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("should write response");
    });

    let output = temp_lock_path("from-url");
    let output_arg = output.to_str().expect("valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["record", "--from-url", &url, "--output", output_arg])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote"))
        .stdout(predicate::str::contains("GET /users"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.starts_with("version: 2\n"));
    assert!(lock.contains("provenance: observed"));
    assert!(lock.contains("GET /users"));
    assert!(!lock.contains("\"alice\""));
}

#[test]
fn record_from_url_rejects_non_json_response() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
    let address = listener.local_addr().expect("listener should have address");
    let url = format!("http://{}/binary", address);

    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("should accept");
        let mut buf = Vec::new();
        while !buf.ends_with(b"\r\n\r\n") {
            let mut byte = [0u8; 1];
            stream.read_exact(&mut byte).expect("should read headers");
            buf.push(byte[0]);
        }
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello"
        )
        .expect("should write response");
    });

    let output = temp_lock_path("from-url-non-json");
    let output_arg = output.to_str().expect("valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["record", "--from-url", &url, "--output", output_arg])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("not JSON"));

    assert!(!output.exists());
}

#[test]
fn init_creates_lock_and_ci_workflow() {
    let output = temp_lock_path("init");
    let output_arg = output.to_str().expect("valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["init", "--output", output_arg])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"))
        .stdout(predicate::str::contains("Next steps:"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.contains("version: 4"));
    assert!(lock.contains("apis: {}"));

    let workflow = std::path::Path::new(".github/workflows/apiwatch.yml");
    assert!(workflow.exists(), "CI workflow should be created");
    let wf = fs::read_to_string(workflow).expect("workflow should be readable");
    fs::remove_file(workflow).ok();
    let _ = std::fs::remove_dir(".github/workflows");
    let _ = std::fs::remove_dir(".github");

    assert!(wf.contains("apiwatch"));
    assert!(wf.contains("push:"));
}

#[test]
fn init_refuses_to_overwrite_existing_lock() {
    let output = temp_lock_path("init-exists");
    let output_arg = output.to_str().expect("valid UTF-8");
    fs::write(&output, "existing content").expect("should write existing file");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["init", "--output", output_arg])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("already exists"));

    fs::remove_file(&output).ok();
}

#[test]
fn coverage_reports_hardened_and_lenient_fields() {
    let output = temp_lock_path("coverage");
    let output_arg = output.to_str().expect("valid UTF-8");

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

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["coverage", "--lock", output_arg])
        .assert()
        .success()
        .stdout(predicate::str::contains("portfolio"))
        .stdout(predicate::str::contains("threshold"))
        .stdout(predicate::str::contains("observations"))
        .stdout(predicate::str::contains("Fields:"))
        .stdout(predicate::str::contains("lenient"))
        .stdout(predicate::str::contains("below floor"));

    fs::remove_file(&output).ok();
}

#[test]
fn coverage_with_name_filter_reports_single_entry() {
    let output = temp_lock_path("coverage-name");
    let output_arg = output.to_str().expect("valid UTF-8");

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

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["coverage", "--lock", output_arg, "--name", "portfolio"])
        .assert()
        .success()
        .stdout(predicate::str::contains("portfolio"));

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["coverage", "--lock", output_arg, "--name", "nonexistent"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("not found"));

    fs::remove_file(&output).ok();
}

#[test]
fn coverage_no_observed_entries_reports_clean() {
    let output = temp_lock_path("coverage-empty");
    let output_arg = output.to_str().expect("valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["init", "--output", output_arg])
        .assert()
        .success();

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["coverage", "--lock", output_arg])
        .assert()
        .success()
        .stdout(predicate::str::contains("no observed entries"));

    fs::remove_file(&output).ok();
}
