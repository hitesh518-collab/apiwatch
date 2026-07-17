use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;

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
