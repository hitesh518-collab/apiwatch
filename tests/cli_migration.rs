use std::io::Write;

use assert_cmd::Command;

#[test]
fn migrate_v2_loads_and_updates() {
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    let fixture = include_str!("../testdata/migration/v2_fixture.lock");
    tmp.write_all(fixture.as_bytes()).expect("write v2 fixture");
    tmp.flush().expect("flush");

    let lock = apiwatch::lockfile::load(tmp.path()).expect("v2 lock should load");
    let rendered = apiwatch::lockfile::render(&lock).expect("v2 lock should render");
    assert!(
        rendered.contains("version: 2"),
        "rerendered v2 lock should contain version marker"
    );

    let update_path = tempfile::NamedTempFile::new()
        .expect("tempfile")
        .into_temp_path();
    update_path.as_os_str().to_str().expect("UTF-8 temp path");

    std::fs::write(&update_path, fixture).expect("write v2 fixture to update target");

    Command::cargo_bin("apiwatch")
        .expect("binary")
        .args([
            "lock",
            "testdata/openapi/verify_matching.yaml",
            "--name",
            "demo",
            "--output",
            update_path.as_os_str().to_str().expect("UTF-8"),
            "--update",
        ])
        .assert()
        .success();
}

#[test]
fn migrate_v3_loads_and_updates() {
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    let fixture = include_str!("../testdata/migration/v3_fixture.lock");
    tmp.write_all(fixture.as_bytes()).expect("write v3 fixture");
    tmp.flush().expect("flush");

    let lock = apiwatch::lockfile::load(tmp.path()).expect("v3 lock should load");
    let rendered = apiwatch::lockfile::render(&lock).expect("v3 lock should render");
    assert!(
        rendered.contains("version: 3"),
        "rerendered v3 lock should contain version marker"
    );

    let update_path = tempfile::NamedTempFile::new()
        .expect("tempfile")
        .into_temp_path();
    std::fs::write(&update_path, fixture).expect("write v3 fixture to update target");

    Command::cargo_bin("apiwatch")
        .expect("binary")
        .args([
            "lock",
            "testdata/openapi/v3_scoped.yaml",
            "--name",
            "demo-v3",
            "--output",
            update_path.as_os_str().to_str().expect("UTF-8"),
            "--update",
        ])
        .assert()
        .success();

    let updated = std::fs::read_to_string(&update_path).expect("read updated lock");
    assert!(
        updated.contains("version: 4"),
        "updated lock should be v4, got:\n{}",
        updated
    );
}
