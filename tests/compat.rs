use std::ffi::OsString;
use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;

fn corpus_file(filename: &str) -> PathBuf {
    let root =
        std::env::var_os("APIWATCH_COMPAT_DIR").unwrap_or_else(|| OsString::from(".compat-cache"));
    let path = PathBuf::from(root).join(filename);
    assert!(
        path.is_file(),
        "missing compatibility fixture {}; run python scripts/fetch_compat_specs.py",
        path.display()
    );
    path
}

fn assert_clean_self_diff(filename: &str) {
    let path = corpus_file(filename);
    let path = path.to_str().expect("compatibility path should be UTF-8");
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["diff", path, path])
        .assert()
        .success()
        .stdout("No changes detected.\n")
        .stderr(predicate::str::is_empty());
}

fn assert_known_failure(filename: &str, expected_error: &str) {
    let path = corpus_file(filename);
    let path = path.to_str().expect("compatibility path should be UTF-8");
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["diff", path, path])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(expected_error));
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn github_rest_is_compatible() {
    assert_clean_self_diff("github.json");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn asana_is_compatible() {
    assert_clean_self_diff("asana.yaml");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn box_is_compatible() {
    assert_clean_self_diff("box.json");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn stripe_reproduces_known_recursive_schema_failure() {
    assert_known_failure(
        "stripe.json",
        "circular schema reference detected: #/components/schemas/file",
    );
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn digitalocean_reproduces_known_metadata_failure() {
    assert_known_failure(
        "digitalocean.yaml",
        "tags[0].description: invalid type: map, expected a string",
    );
}
