use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_reports_the_crate_version() {
    Command::cargo_bin("apiwatch")
        .expect("binary")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^apiwatch \d+\.\d+\.\d+( \([0-9a-f]+\))?\n?$").unwrap());
}
