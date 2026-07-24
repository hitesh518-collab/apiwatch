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

#[test]
fn lock_writes_deterministic_single_api_lockfile() {
    let output_path = temp_lock_path("single-api");
    let output_arg = output_path
        .to_str()
        .expect("temp path should be valid UTF-8");

    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");
    command
        .args([
            "lock",
            "testdata/openapi/lock_ordering.yaml",
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
        "\
version: 1
apis:
  users:
    source: openapi
    operations:
    - method: GET
      path: /users
    - method: POST
      path: /users
"
    );
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
