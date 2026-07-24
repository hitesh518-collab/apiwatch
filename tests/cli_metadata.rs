use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_reports_the_crate_version() {
    Command::cargo_bin("apiwatch")
        .expect("binary")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("apiwatch 0.7.0"));
}
