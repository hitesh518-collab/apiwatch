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
    let mut path = std::env::temp_dir();
    path.push(format!(
        "apiwatch-{name}-{}-{suffix}.lock",
        std::process::id()
    ));
    path
}

fn canonical_lines(value: &str) -> String {
    value.replace("\r\n", "\n")
}

#[test]
fn lock_creates_a_deterministic_v3_file() {
    let output_path = temp_lock_path("single-api");
    let output_arg = output_path
        .to_str()
        .expect("temp path should be valid UTF-8");

    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");
    command
        .args([
            "lock",
            "testdata/openapi/verify_matching.yaml",
            "--name",
            "users",
            "--output",
            output_arg,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Wrote {}",
            output_path.display()
        )));

    let rendered = fs::read_to_string(&output_path).expect("lockfile should be written");
    fs::remove_file(&output_path).ok();

    assert_eq!(
        rendered,
        canonical_lines(
            &fs::read_to_string("testdata/lock/v3_users.lock")
                .expect("golden v3 lockfile should exist")
        )
    );
}

#[test]
fn lock_requires_update_and_preserves_existing_bytes() {
    let output = temp_lock_path("requires-update");
    fs::write(&output, "preserve").expect("existing lock should write");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            "testdata/openapi/verify_matching.yaml",
            "--name",
            "users",
            "--output",
            output.to_str().expect("temp path should be valid UTF-8"),
        ])
        .assert()
        .code(2);

    assert_eq!(
        fs::read_to_string(&output).expect("existing lock should remain readable"),
        "preserve"
    );
    fs::remove_file(output).ok();
}

#[test]
fn lock_update_preserves_observed_entries() {
    let output = temp_lock_path("migration");
    fs::copy("testdata/lock/v2_declared_observed.lock", &output)
        .expect("migration fixture should copy");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            "testdata/openapi/verify_matching.yaml",
            "--name",
            "users",
            "--output",
            output.to_str().expect("temp path should be valid UTF-8"),
            "--update",
        ])
        .assert()
        .success();

    let rendered = fs::read_to_string(&output).expect("updated lock should be readable");
    fs::remove_file(output).ok();
    assert!(rendered.starts_with("version: 3\n"));
    assert!(rendered.contains("provenance: observed"));
}

#[test]
fn lock_stores_exact_scope_and_rejects_missing_selectors() {
    let output = temp_lock_path("scoped");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            "testdata/openapi/v3_scoped.yaml",
            "--name",
            "scoped",
            "--output",
            output.to_str().expect("temp path should be valid UTF-8"),
            "--include-operation",
            "GET /users",
        ])
        .assert()
        .success();

    let rendered = fs::read_to_string(&output).expect("scoped lock should be readable");
    fs::remove_file(output).ok();
    assert!(rendered.contains("operations:\n      - GET /users"));

    let missing = temp_lock_path("missing-selector");
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            "testdata/openapi/v3_scoped.yaml",
            "--name",
            "scoped",
            "--output",
            missing.to_str().expect("temp path should be valid UTF-8"),
            "--include-operation",
            "DELETE /missing",
        ])
        .assert()
        .code(2);
    assert!(!missing.exists());
}

#[test]
fn lock_accepts_exact_scopes_spanning_multiple_http_methods() {
    let output = temp_lock_path("multi-method-scope");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            "testdata/openapi/v3_scoped.yaml",
            "--name",
            "scoped",
            "--output",
            output.to_str().expect("temp path should be valid UTF-8"),
            "--include-operation",
            "DELETE /users/{id}",
            "--include-operation",
            "GET /users",
        ])
        .assert()
        .success();

    let rendered = fs::read_to_string(&output).expect("scoped lock should be readable");
    fs::remove_file(output).ok();
    assert!(rendered.contains("operations:\n      - GET /users\n      - DELETE /users/{id}"));
}

#[test]
fn lock_size_failure_preserves_existing_bytes() {
    let output = temp_lock_path("size-failure");
    fs::write(&output, "preserve").expect("existing lock should write");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            "testdata/openapi/verify_matching.yaml",
            "--name",
            "users",
            "--output",
            output.to_str().expect("temp path should be valid UTF-8"),
            "--max-lock-bytes",
            "1",
            "--update",
        ])
        .assert()
        .code(2);

    assert_eq!(
        fs::read_to_string(&output).expect("existing lock should remain readable"),
        "preserve"
    );
    fs::remove_file(output).ok();
}

#[test]
fn lock_exits_two_for_empty_api_name() {
    let output_path = temp_lock_path("empty-name");
    let output_arg = output_path
        .to_str()
        .expect("temp path should be valid UTF-8");

    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");
    command
        .args([
            "lock",
            "testdata/openapi/lock_ordering.yaml",
            "--name",
            "",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("api name cannot be empty"));

    assert!(
        !output_path.exists(),
        "lockfile should not be written when the api name is invalid"
    );
}

#[test]
fn lock_exits_two_for_invalid_openapi_input() {
    let output_path = temp_lock_path("invalid-input");
    let output_arg = output_path
        .to_str()
        .expect("temp path should be valid UTF-8");

    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");
    command
        .args([
            "lock",
            "testdata/openapi/invalid_yaml.yaml",
            "--name",
            "users",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("failed to parse OpenAPI YAML"));

    assert!(
        !output_path.exists(),
        "lockfile should not be written when OpenAPI parsing fails"
    );
}

#[test]
fn lock_rejects_openapi_31_with_an_accurate_message() {
    let output = temp_lock_path("unsupported-31");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            "testdata/openapi/unsupported_31.yaml",
            "--name",
            "users",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("OpenAPI 3.1 is not yet supported"));

    fs::remove_file(output).ok();
}
