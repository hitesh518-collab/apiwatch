use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};

fn parse_json_output(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be JSON")
}

fn sarif_rule_ids(rendered: &Value) -> Vec<&str> {
    rendered["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("SARIF rules should be an array")
        .iter()
        .map(|rule| rule["id"].as_str().expect("SARIF rule should have an ID"))
        .collect()
}

fn serve_once(status: &str, content_type: &str, body: &'static str, suffix: &str) -> String {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
    let address = listener
        .local_addr()
        .expect("test server should have an address");
    let status = status.to_string();
    let content_type = content_type.to_string();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("test server should accept");
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            assert!(
                request.len() < 8 * 1024,
                "test server request headers exceed 8 KiB"
            );
            let mut byte = [0_u8; 1];
            stream
                .read_exact(&mut byte)
                .expect("test server should read request headers");
            request.push(byte[0]);
        }
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("test server should write response");
    });
    format!("http://{address}/{suffix}")
}

struct ProxyProbe {
    url: String,
    connection: std::sync::mpsc::Receiver<bool>,
}

impl ProxyProbe {
    fn assert_not_used(self) {
        match self
            .connection
            .recv_timeout(std::time::Duration::from_secs(3))
            .expect("proxy probe should finish")
        {
            false => {}
            true => panic!("Verify unexpectedly connected to the configured HTTP proxy"),
        }
    }
}

fn serve_proxy_probe(body: &'static str) -> ProxyProbe {
    use std::io::{ErrorKind, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    let listener = TcpListener::bind("127.0.0.1:0").expect("proxy probe should bind");
    listener
        .set_nonblocking(true)
        .expect("proxy probe should become nonblocking");
    let address = listener
        .local_addr()
        .expect("proxy probe should have an address");
    let (connection_sender, connection) = mpsc::sync_channel(1);

    thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut request = Vec::new();
                    while !request.ends_with(b"\r\n\r\n") && request.len() < 8 * 1024 {
                        let mut byte = [0_u8; 1];
                        if stream.read_exact(&mut byte).is_err() {
                            break;
                        }
                        request.push(byte[0]);
                    }
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/yaml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = connection_sender.send(true);
                    return;
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        let _ = connection_sender.send(false);
                        return;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => {
                    let _ = connection_sender.send(false);
                    return;
                }
            }
        }
    });

    ProxyProbe {
        url: format!("http://{address}"),
        connection,
    }
}

fn verify_command(openapi: &str, name: &str, lock: &str) -> Command {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");
    command.args(["verify", openapi, "--name", name, "--lock", lock]);
    command
}

fn lock_from_v4(openapi: &str, name: &str) -> PathBuf {
    let lock = observed_lock_path();
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            openapi,
            "--name",
            name,
            "--output",
            lock.to_str().expect("temp path should be valid UTF-8"),
        ])
        .assert()
        .success();
    lock
}

fn lock_from_v3(openapi: &str, name: &str) -> PathBuf {
    let lock = observed_lock_path();
    let contract = apiwatch::openapi::load_contract(Path::new(openapi))
        .expect("v3 fixture contract should load");
    let scope = apiwatch::lockfile::scope_from_selectors(&[]).expect("full v3 scope should build");
    let entry = apiwatch::lockfile::build_v3_declared(
        &contract,
        scope,
        apiwatch::lockfile::DEFAULT_MAX_LOCK_BYTES,
    )
    .expect("v3 declared entry should build");
    let lockfile = apiwatch::lockfile::new_v3(name, entry).expect("v3 lock should build");
    fs::write(
        &lock,
        apiwatch::lockfile::render(&lockfile).expect("v3 lock should render"),
    )
    .expect("v3 lock should write");
    lock
}

#[test]
fn phase2_d08_v4_verify_matches_authentication_by_wire_identity() {
    let lock = lock_from_v4("testdata/openapi/phase2_d08_auth_identity_old.yaml", "d08");

    verify_command(
        "testdata/openapi/phase2_d08_auth_identity_new.yaml",
        "d08",
        lock.to_str().expect("temp path should be valid UTF-8"),
    )
    .assert()
    .code(1)
    .stdout(predicate::str::contains(
        "GET /keyed: authentication apiKeyAuth changed identity",
    ))
    .stdout(predicate::str::contains("GET /renamed").not());

    fs::remove_file(lock).ok();
}

#[test]
fn phase2_d08_v3_verify_retains_label_based_authentication_matching() {
    let lock = lock_from_v3("testdata/openapi/phase2_d08_auth_identity_old.yaml", "d08");

    verify_command(
        "testdata/openapi/phase2_d08_auth_identity_new.yaml",
        "d08",
        lock.to_str().expect("temp path should be valid UTF-8"),
    )
    .assert()
    .code(1)
    .stdout(predicate::str::contains(
        "GET /renamed: authentication accessToken (bearer) added",
    ));

    fs::remove_file(lock).ok();
}

#[test]
fn phase2_d08_v4_allows_distinct_unresolved_authentication_labels() {
    let document = tempfile::Builder::new()
        .suffix(".yaml")
        .tempfile()
        .expect("temporary OpenAPI document should be created");
    fs::write(
        document.path(),
        "openapi: 3.0.3\ninfo: { title: D-08 unresolved, version: '1' }\npaths:\n  /users:\n    get:\n      security:\n        - firstUnknown: []\n          secondUnknown: []\n      responses: { '200': { description: OK } }\n",
    )
    .expect("temporary OpenAPI document should be written");
    let lock = observed_lock_path();

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            document
                .path()
                .to_str()
                .expect("temporary path should be UTF-8"),
            "--name",
            "unresolved",
            "--output",
            lock.to_str().expect("temporary path should be UTF-8"),
        ])
        .assert()
        .success();

    verify_command(
        document
            .path()
            .to_str()
            .expect("temporary path should be UTF-8"),
        "unresolved",
        lock.to_str().expect("temporary path should be UTF-8"),
    )
    .assert()
    .success();

    fs::remove_file(lock).ok();
}

#[test]
fn phase2_d08_rejects_known_identity_duplicates_across_security_alternatives() {
    let document = tempfile::Builder::new()
        .suffix(".yaml")
        .tempfile()
        .expect("temporary OpenAPI document should be created");
    fs::write(
        document.path(),
        "openapi: 3.0.3\ninfo: { title: D-08 alternatives, version: '1' }\ncomponents:\n  securitySchemes:\n    readOAuth:\n      type: oauth2\n      flows:\n        password:\n          tokenUrl: https://auth.example.test/token\n          scopes: { read: read, write: write }\n    writeOAuth:\n      type: oauth2\n      flows:\n        password:\n          tokenUrl: https://auth.example.test/token\n          scopes: { read: read, write: write }\npaths:\n  /users:\n    get:\n      security:\n        - readOAuth: [read]\n        - writeOAuth: [write]\n      responses: { '200': { description: OK } }\n",
    )
    .expect("temporary OpenAPI document should be written");
    let input = document
        .path()
        .to_str()
        .expect("temporary path should be UTF-8");
    let lock = observed_lock_path();
    let lock_path = lock.to_str().expect("temporary lock path should be UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["diff", input, input])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "duplicate authentication identity",
        ));
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            input,
            "--name",
            "alternatives",
            "--output",
            lock_path,
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "duplicate authentication identity",
        ));

    fs::remove_file(lock).ok();
}

#[test]
fn phase2_d08_v4_verify_deduplicates_source_authentication_scopes() {
    let old = tempfile::Builder::new()
        .suffix(".yaml")
        .tempfile()
        .expect("old OpenAPI document should be created");
    let new = tempfile::Builder::new()
        .suffix(".yaml")
        .tempfile()
        .expect("new OpenAPI document should be created");
    let template = |scopes: &str| {
        format!(
        "openapi: 3.0.3\ninfo: {{ title: D-08 scopes, version: '1' }}\ncomponents:\n  securitySchemes:\n    oauth:\n      type: oauth2\n      flows:\n        password:\n          tokenUrl: https://auth.example.test/token\n          scopes: {{ read: read, write: write }}\npaths:\n  /users:\n    get:\n      security:\n        - oauth: [{scopes}]\n      responses: {{ '200': {{ description: OK }} }}\n"
    )
    };
    fs::write(old.path(), template("write, read, write")).expect("old document should write");
    fs::write(new.path(), template("read")).expect("new document should write");
    let lock = lock_from_v4(
        old.path().to_str().expect("old path should be UTF-8"),
        "scopes",
    );

    let diff = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            old.path().to_str().expect("old path should be UTF-8"),
            new.path().to_str().expect("new path should be UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("diff command should run");
    let verify = verify_command(
        new.path().to_str().expect("new path should be UTF-8"),
        "scopes",
        lock.to_str().expect("lock path should be UTF-8"),
    )
    .args(["--format", "json"])
    .output()
    .expect("verify command should run");
    fs::remove_file(lock).ok();

    assert_eq!(diff.status.code(), Some(0));
    assert_eq!(verify.status.code(), Some(0));
    assert_eq!(
        parse_json_output(&diff)["changes"],
        json!([{
            "severity": "non_breaking",
            "method": "GET",
            "path": "/users",
            "message": "authentication oauth scope write removed"
        }])
    );
    assert_eq!(
        parse_json_output(&verify)["changes"],
        parse_json_output(&diff)["changes"]
    );
}

fn scoped_lock_v3(selector: &str) -> PathBuf {
    let lock = observed_lock_path();
    let selectors = vec![selector.to_owned()];
    let contract = apiwatch::openapi::load_contract(Path::new("testdata/openapi/v3_scoped.yaml"))
        .expect("scoped v3 fixture contract should load");
    let scoped = apiwatch::lock_size::scope_contract(&contract, &selectors)
        .expect("scoped v3 contract should build");
    let scope =
        apiwatch::lockfile::scope_from_selectors(&selectors).expect("scoped v3 scope should build");
    let entry = apiwatch::lockfile::build_v3_declared(
        &scoped,
        scope,
        apiwatch::lockfile::DEFAULT_MAX_LOCK_BYTES,
    )
    .expect("scoped v3 declared entry should build");
    let lockfile =
        apiwatch::lockfile::new_v3("scoped", entry).expect("scoped v3 lock should build");
    fs::write(
        &lock,
        apiwatch::lockfile::render(&lockfile).expect("scoped v3 lock should render"),
    )
    .expect("scoped v3 lock should write");
    lock
}

fn scoped_lock_v3_from(openapi: &str, selector: &str) -> PathBuf {
    let lock = observed_lock_path();
    let selectors = vec![selector.to_owned()];
    let contract = apiwatch::openapi::load_contract(Path::new(openapi))
        .expect("scoped v3 fixture contract should load");
    let scoped = apiwatch::lock_size::scope_contract(&contract, &selectors)
        .expect("scoped v3 contract should build");
    let scope =
        apiwatch::lockfile::scope_from_selectors(&selectors).expect("scoped v3 scope should build");
    let entry = apiwatch::lockfile::build_v3_declared(
        &scoped,
        scope,
        apiwatch::lockfile::DEFAULT_MAX_LOCK_BYTES,
    )
    .expect("scoped v3 declared entry should build");
    let lockfile = apiwatch::lockfile::new_v3("d07", entry).expect("scoped v3 lock should build");
    fs::write(
        &lock,
        apiwatch::lockfile::render(&lockfile).expect("scoped v3 lock should render"),
    )
    .expect("scoped v3 lock should write");
    lock
}

fn observed_lock_path() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "apiwatch-observed-verify-{}-{suffix}.lock",
        std::process::id()
    ))
}

#[test]
fn phase2_d07_verify_scoped_v4_lock_matches_renamed_path_placeholders() {
    let lock = observed_lock_path();
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            "testdata/openapi/phase2_d07_path_template_old.yaml",
            "--name",
            "d07",
            "--output",
            lock.to_str().expect("temporary path should be valid UTF-8"),
            "--include-operation",
            "GET /users/{userId}/orders/{orderId}",
        ])
        .assert()
        .success();

    verify_command(
        "testdata/openapi/phase2_d07_path_template_new.yaml",
        "d07",
        lock.to_str().expect("temporary path should be valid UTF-8"),
    )
    .assert()
    .success()
    .stdout("Verified d07\n");
    fs::remove_file(lock).ok();
}

#[test]
fn phase2_d07_verify_scoped_v3_lock_matches_renamed_path_placeholders() {
    let lock = scoped_lock_v3_from(
        "testdata/openapi/phase2_d07_path_template_old.yaml",
        "GET /users/{userId}/orders/{orderId}",
    );

    verify_command(
        "testdata/openapi/phase2_d07_path_template_new.yaml",
        "d07",
        lock.to_str().expect("temporary path should be valid UTF-8"),
    )
    .assert()
    .success()
    .stdout("Verified d07\n");
    fs::remove_file(lock).ok();
}

#[test]
fn phase2_d07_verify_legacy_v1_and_v2_locks_use_path_template_identity() {
    for contents in [
        "version: 1\napis:\n  d07:\n    source: openapi\n    operations:\n      - method: GET\n        path: /users/{userId}/orders/{orderId}\n",
        "version: 2\napis:\n  d07:\n    provenance: declared\n    source: openapi\n    operations:\n      - method: GET\n        path: /users/{userId}/orders/{orderId}\n",
    ] {
        let lock = observed_lock_path();
        fs::write(&lock, contents).expect("legacy lock should be written");

        verify_command(
            "testdata/openapi/phase2_d07_path_template_old.yaml",
            "d07",
            lock.to_str().expect("temporary path should be valid UTF-8"),
        )
        .assert()
        .success()
        .stdout("Verified d07\n");
        fs::remove_file(lock).ok();
    }
}

#[test]
fn phase2_d01_verify_v4_matches_diff_request_body_findings() {
    let lock = lock_from_v4("testdata/openapi/phase2_d01_request_body_old.yaml", "d01");
    let lock = lock.to_str().expect("temp path should be valid UTF-8");
    let verify_output = verify_command(
        "testdata/openapi/phase2_d01_request_body_new.yaml",
        "d01",
        lock,
    )
    .args(["--format", "json"])
    .output()
    .expect("Verify command should run");
    let diff_output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/phase2_d01_request_body_old.yaml",
            "testdata/openapi/phase2_d01_request_body_new.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");
    fs::remove_file(lock).ok();

    assert_eq!(verify_output.status.code(), Some(1));
    assert_eq!(diff_output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&verify_output)["changes"],
        parse_json_output(&diff_output)["changes"]
    );
    assert_eq!(
        parse_json_output(&verify_output)["changes"],
        json!([
            {
                "severity": "non_breaking",
                "method": "POST",
                "path": "/optional-added",
                "message": "request body added as optional"
            },
            {
                "severity": "breaking",
                "method": "POST",
                "path": "/required-added",
                "message": "request body added as required"
            },
            {
                "severity": "breaking",
                "method": "POST",
                "path": "/requiredness",
                "message": "request body changed from optional to required"
            }
        ])
    );
}

#[test]
fn phase2_d09_verify_v4_and_v3_preserve_composition_findings() {
    let expected = [
        "GET /allof-required-change: response 200 application/json field name changed from required to optional",
        "GET /response-branch-addition: response 200 application/json field anyOf[1] added",
        "POST /request-branch-removal: request application/json field oneOf[1] removed",
    ];
    for (name, lock) in [
        (
            "d09-v4",
            lock_from_v4("testdata/openapi/phase2_d09_composition_old.yaml", "d09-v4"),
        ),
        (
            "d09-v3",
            lock_from_v3("testdata/openapi/phase2_d09_composition_old.yaml", "d09-v3"),
        ),
    ] {
        let output = verify_command(
            "testdata/openapi/phase2_d09_composition_new.yaml",
            name,
            lock.to_str().expect("temp path should be valid UTF-8"),
        )
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
        let text = String::from_utf8(output).expect("verify output should be UTF-8");
        for finding in expected {
            assert!(text.contains(finding), "missing {finding}: {text}");
        }
        assert!(
            !text.contains("/allof-reordered"),
            "reordered allOf must be silent: {text}"
        );
        assert!(
            !text.contains("/allof-empty-neutral"),
            "empty allOf branch must be neutral: {text}"
        );
        assert!(
            !text.contains("/oneof-reordered"),
            "reordered oneOf must be silent: {text}"
        );
        assert!(
            !text.contains("/anyof-reordered"),
            "reordered anyOf must be silent: {text}"
        );
        assert!(
            !text.contains("/enum-branch-dedup"),
            "semantic duplicate anyOf branches must be silent: {text}"
        );
    }
}

#[test]
fn phase2_d10_verify_v4_and_v3_preserve_array_item_findings() {
    let diff = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/phase2_d10_array_items_old.yaml",
            "testdata/openapi/phase2_d10_array_items_new.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("diff command should run");
    assert_eq!(diff.status.code(), Some(1));
    let expected = json!([
        { "severity": "warning", "method": "GET", "path": "/nested", "message": "response 200 application/json field items.items format changed from uuid to date-time" },
        { "severity": "breaking", "method": "GET", "path": "/users", "message": "response 200 application/json field items.name removed" },
        { "severity": "breaking", "method": "POST", "path": "/users", "message": "request application/json field items.email added as required" }
    ]);
    assert_eq!(parse_json_output(&diff)["changes"], expected);

    for (name, lock) in [
        (
            "d10-v4",
            lock_from_v4("testdata/openapi/phase2_d10_array_items_old.yaml", "d10-v4"),
        ),
        (
            "d10-v3",
            lock_from_v3("testdata/openapi/phase2_d10_array_items_old.yaml", "d10-v3"),
        ),
    ] {
        let output = verify_command(
            "testdata/openapi/phase2_d10_array_items_new.yaml",
            name,
            lock.to_str().expect("temp path should be valid UTF-8"),
        )
        .args(["--format", "json"])
        .output()
        .expect("verify command should run");
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(parse_json_output(&output)["changes"], expected);
        assert_eq!(
            parse_json_output(&output)["changes"],
            parse_json_output(&diff)["changes"]
        );
        fs::remove_file(lock).ok();
    }
}

#[test]
fn phase2_d10_loads_and_verifies_pre_task_v4_lock() {
    verify_command(
        "testdata/openapi/privacy_sentinels.yaml",
        "private",
        "testdata/lock/v4_private.lock",
    )
    .assert()
    .success();
}

#[test]
fn phase2_d02_verify_v4_matches_diff_content_type_findings() {
    let lock = lock_from_v4("testdata/openapi/phase2_d02_content_type_old.yaml", "d02");
    let lock = lock.to_str().expect("temp path should be valid UTF-8");
    let verify_output = verify_command(
        "testdata/openapi/phase2_d02_content_type_new.yaml",
        "d02",
        lock,
    )
    .args(["--format", "json"])
    .output()
    .expect("Verify command should run");
    let diff_output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/phase2_d02_content_type_old.yaml",
            "testdata/openapi/phase2_d02_content_type_new.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");
    fs::remove_file(lock).ok();

    assert_eq!(verify_output.status.code(), Some(1));
    assert_eq!(diff_output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&verify_output)["changes"],
        parse_json_output(&diff_output)["changes"]
    );
    assert_eq!(
        parse_json_output(&verify_output)["changes"],
        json!([
            {
                "severity": "breaking",
                "method": "GET",
                "path": "/response",
                "message": "response 200 content type application/problem+json added"
            },
            {
                "severity": "breaking",
                "method": "POST",
                "path": "/request",
                "message": "request content type application/json removed"
            },
            {
                "severity": "non_breaking",
                "method": "POST",
                "path": "/request",
                "message": "request content type application/xml added"
            }
        ])
    );
}

#[test]
fn phase2_d03_verify_v4_matches_diff_response_requiredness_findings() {
    let lock = lock_from_v4(
        "testdata/openapi/phase2_d03_response_required_old.yaml",
        "d03",
    );
    let lock = lock.to_str().expect("temp path should be valid UTF-8");
    let verify_output = verify_command(
        "testdata/openapi/phase2_d03_response_required_new.yaml",
        "d03",
        lock,
    )
    .args(["--format", "json"])
    .output()
    .expect("Verify command should run");
    let diff_output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/phase2_d03_response_required_old.yaml",
            "testdata/openapi/phase2_d03_response_required_new.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");
    fs::remove_file(lock).ok();

    assert_eq!(verify_output.status.code(), Some(1));
    assert_eq!(diff_output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&verify_output)["changes"],
        parse_json_output(&diff_output)["changes"]
    );
    assert_eq!(
        parse_json_output(&verify_output)["changes"],
        json!([
            {
                "severity": "breaking",
                "method": "GET",
                "path": "/users",
                "message": "response 200 application/json field id changed from required to optional"
            },
            {
                "severity": "non_breaking",
                "method": "GET",
                "path": "/users",
                "message": "response 200 application/json field name changed from optional to required"
            }
        ])
    );
}

#[test]
fn phase2_d04_verify_v4_matches_diff_additional_properties_findings() {
    let lock = lock_from_v4(
        "testdata/openapi/phase2_d04_additional_properties_old.yaml",
        "d04",
    );
    let lock = lock.to_str().expect("temp path should be valid UTF-8");
    let verify_output = verify_command(
        "testdata/openapi/phase2_d04_additional_properties_new.yaml",
        "d04",
        lock,
    )
    .args(["--format", "json"])
    .output()
    .expect("Verify command should run");
    let diff_output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/phase2_d04_additional_properties_old.yaml",
            "testdata/openapi/phase2_d04_additional_properties_new.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");
    fs::remove_file(lock).ok();

    assert_eq!(verify_output.status.code(), Some(1));
    assert_eq!(diff_output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&verify_output)["changes"],
        parse_json_output(&diff_output)["changes"]
    );
}

#[test]
fn phase2_d04_verify_rejects_unknown_additional_properties_wire_fields() {
    let lock = lock_from_v4(
        "testdata/openapi/phase2_d04_additional_properties_old.yaml",
        "d04",
    );
    let rendered = fs::read_to_string(&lock).expect("v4 lock should be readable");
    let marker = "additional_properties:\n            kind: any";
    let tampered = rendered.replacen(
        marker,
        "additional_properties:\n            kind: any\n            unexpected_field: accepted",
        1,
    );
    assert_ne!(tampered, rendered, "fixture should include an any policy");
    fs::write(&lock, tampered).expect("tampered v4 lock should write");
    let lock = lock.to_str().expect("temp path should be valid UTF-8");

    verify_command(
        "testdata/openapi/phase2_d04_additional_properties_old.yaml",
        "d04",
        lock,
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains(
        "unknown field in additionalProperties policy",
    ));

    fs::remove_file(lock).ok();
}

#[test]
fn phase2_d05_verify_v4_matches_diff_format_findings() {
    let lock = lock_from_v4("testdata/openapi/phase2_d05_format_old.yaml", "d05");
    let lock = lock.to_str().expect("temp path should be valid UTF-8");
    let verify_output = verify_command("testdata/openapi/phase2_d05_format_new.yaml", "d05", lock)
        .args(["--format", "json"])
        .output()
        .expect("Verify command should run");
    let diff_output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/phase2_d05_format_old.yaml",
            "testdata/openapi/phase2_d05_format_new.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");
    fs::remove_file(lock).ok();

    assert_eq!(verify_output.status.code(), Some(0));
    assert_eq!(diff_output.status.code(), Some(0));
    assert_eq!(
        parse_json_output(&verify_output)["changes"],
        parse_json_output(&diff_output)["changes"]
    );
    assert_eq!(
        parse_json_output(&verify_output)["changes"],
        json!([
            {
                "severity": "warning",
                "method": "GET",
                "path": "/events",
                "message": "response 200 application/json field created_at format changed from date to date-time"
            },
            {
                "severity": "warning",
                "method": "POST",
                "path": "/users",
                "message": "request application/json field count format changed from int32 to int64"
            },
            {
                "severity": "warning",
                "method": "POST",
                "path": "/users",
                "message": "request application/json field id format changed from none to uuid"
            }
        ])
    );
}

#[test]
fn phase2_d06_verify_v4_matches_diff_effective_server_findings() {
    let lock = lock_from_v4("testdata/openapi/phase2_d06_servers_old.yaml", "d06");
    let lock = lock.to_str().expect("temp path should be valid UTF-8");
    let verify_output = verify_command("testdata/openapi/phase2_d06_servers_new.yaml", "d06", lock)
        .args(["--format", "json"])
        .output()
        .expect("Verify command should run");
    let diff_output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/phase2_d06_servers_old.yaml",
            "testdata/openapi/phase2_d06_servers_new.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(verify_output.status.code(), Some(1));
    assert_eq!(diff_output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&verify_output)["changes"],
        parse_json_output(&diff_output)["changes"]
    );
}

#[test]
fn verify_v3_json_reports_partial_phase_two_coverage() {
    let output = verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/v3_users.lock",
    )
    .args(["--format", "json"])
    .output()
    .unwrap();
    let rendered: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rendered["coverage"], "partial");
    assert_eq!(rendered["limitations"][0]["code"], "phase2_relock_required");
}

#[test]
fn verify_v3_text_and_sarif_report_the_phase_two_relock_limitation() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/v3_users.lock",
    )
    .assert()
    .stderr(predicate::str::contains(
        "api.lock v3 lacks Phase 2 contract fields; re-lock from the original OpenAPI source for full coverage",
    ));

    let output = verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/v3_users.lock",
    )
    .args(["--format", "sarif"])
    .output()
    .unwrap();
    let rendered: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        rendered["runs"][0]["invocations"][0]["toolExecutionNotifications"][0]["descriptor"]["id"],
        "apiwatch/phase2-relock-required"
    );
}

#[test]
fn verify_v3_d16_reports_four_breaking_findings() {
    let lock = lock_from_v3("testdata/openapi/v3_d16_old.yaml", "d16");
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "verify",
            "testdata/openapi/v3_d16_new.yaml",
            "--name",
            "d16",
            "--lock",
            lock.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("verify should run");
    fs::remove_file(lock).ok();

    assert_eq!(output.status.code(), Some(1));
    let rendered = parse_json_output(&output);
    assert_eq!(rendered["summary"]["breaking"], 4);
    assert_eq!(
        rendered["changes"]
            .as_array()
            .expect("changes should be an array")
            .iter()
            .map(|change| change["message"]
                .as_str()
                .expect("message should be a string"))
            .collect::<Vec<_>>(),
        vec![
            "authentication bearerAuth (bearer) added",
            "query parameter account_id removed",
            "query parameter account added as required",
            "response status 204 removed",
        ]
    );
}

#[test]
fn verify_v3_scope_ignores_unselected_additions() {
    let lock = scoped_lock_v3("GET /users");

    verify_command(
        "testdata/openapi/v3_scoped_added_unrelated.yaml",
        "scoped",
        lock.to_str().expect("temp path should be valid UTF-8"),
    )
    .assert()
    .success()
    .stdout("Verified scoped\n");

    fs::remove_file(lock).ok();
}

#[test]
fn verify_v3_selected_operation_removal_is_breaking() {
    let lock = scoped_lock_v3("GET /users");

    verify_command(
        "testdata/openapi/v3_scoped_without_users.yaml",
        "scoped",
        lock.to_str().expect("temp path should be valid UTF-8"),
    )
    .assert()
    .code(1)
    .stdout(predicate::str::contains("endpoint removed"));

    fs::remove_file(lock).ok();
}

#[test]
fn verify_v3_warning_only_change_exits_zero() {
    let lock = lock_from_v3("testdata/openapi/status_error_added_old.yaml", "users");

    verify_command(
        "testdata/openapi/status_error_added_new.yaml",
        "users",
        lock.to_str().expect("temp path should be valid UTF-8"),
    )
    .assert()
    .success()
    .stdout(predicate::str::contains("Warnings"));

    fs::remove_file(lock).ok();
}

#[test]
fn verify_v4_json_uses_full_coverage_and_diff_findings() {
    let lock = lock_from_v4("testdata/openapi/v3_d16_old.yaml", "d16");
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "verify",
            "testdata/openapi/v3_d16_new.yaml",
            "--name",
            "d16",
            "--lock",
            lock.to_str().expect("temp path should be valid UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("verify should run");
    fs::remove_file(lock).ok();

    let rendered = parse_json_output(&output);
    assert_eq!(rendered["version"], 2);
    assert_eq!(rendered["coverage"], "full");
    assert_eq!(rendered["limitations"], json!([]));
    assert!(
        rendered["summary"]["breaking"]
            .as_u64()
            .expect("breaking count should be numeric")
            > 0
    );
    assert!(rendered["changes"][0]["message"].is_string());
}

#[test]
fn legacy_json_reports_route_only_limitation_without_stderr_noise() {
    let output = verify_command(
        "testdata/openapi/verify_current.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .arg("--format")
    .arg("json")
    .output()
    .expect("verify should run");

    let rendered = parse_json_output(&output);
    assert_eq!(rendered["coverage"], "routes");
    assert_eq!(rendered["limitations"][0]["code"], "route_only_lock");
    assert!(output.stderr.is_empty());
}

#[test]
fn legacy_text_warns_on_stderr() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .stderr(predicate::str::contains(
        "api.lock v1/v2 declared entry is route-only",
    ));
}

#[test]
fn legacy_sarif_uses_tool_execution_notification() {
    let output = verify_command(
        "testdata/openapi/verify_current.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .arg("--format")
    .arg("sarif")
    .output()
    .expect("verify should run");

    assert!(output.stderr.is_empty());
    let rendered = parse_json_output(&output);
    assert_eq!(
        rendered["runs"][0]["invocations"][0]["toolExecutionNotifications"][0]["descriptor"]["id"],
        "apiwatch/route-only-lock"
    );
}

fn record_portfolio(lock: &Path) {
    let lock = lock.to_str().expect("temp path should be valid UTF-8");
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-json",
            "testdata/observed/portfolio-empty.json",
            "--name",
            "portfolio",
            "--output",
            lock,
        ])
        .assert()
        .success();
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-json",
            "testdata/observed/portfolio-populated.json",
            "--name",
            "portfolio",
            "--output",
            lock,
            "--merge",
        ])
        .assert()
        .success();
}

fn record_map_portfolio(lock: &Path) {
    let lock = lock.to_str().expect("temp path should be valid UTF-8");
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-json",
            "testdata/observed/portfolio-map-initial.json",
            "--name",
            "portfolio",
            "--output",
            lock,
            "--map-at",
            "$.by_broker",
            "--map-at",
            "$.state.by_region",
        ])
        .assert()
        .success();
}

#[test]
fn verify_observed_map_accepts_dynamic_key_churn_and_empty_maps() {
    let lock = observed_lock_path();
    record_map_portfolio(&lock);
    verify_command(
        "testdata/observed/portfolio-map-matching.json",
        "portfolio",
        lock.to_str().expect("temp path should be valid UTF-8"),
    )
    .assert()
    .success()
    .stdout("Verified portfolio\n");

    fs::remove_file(lock).ok();
}

#[test]
fn verify_observed_map_reports_dynamic_value_type_drift_without_values() {
    let lock = observed_lock_path();
    record_map_portfolio(&lock);
    verify_command(
        "testdata/observed/portfolio-map-value-drift.json",
        "portfolio",
        lock.to_str().expect("temp path should be valid UTF-8"),
    )
    .assert()
    .code(1)
    .stdout(predicate::str::contains(
        "BREAKING $.by_broker.<map-value>.pnl_pct: expected number, found string\n",
    ))
    .stdout(predicate::str::contains("verify-secret").not())
    .stdout(predicate::str::contains("acme").not())
    .stdout(predicate::str::contains("globex").not());

    fs::remove_file(lock).ok();
}

#[test]
fn verify_observed_map_reports_map_to_scalar_drift() {
    let lock = observed_lock_path();
    record_map_portfolio(&lock);
    verify_command(
        "testdata/observed/portfolio-map-scalar-drift.json",
        "portfolio",
        lock.to_str().expect("temp path should be valid UTF-8"),
    )
    .assert()
    .code(1)
    .stdout("BREAKING $.by_broker: expected map, found string\n");

    fs::remove_file(lock).ok();
}

#[test]
fn verify_observed_map_json_and_sarif_report_only_paths_and_shape_names() {
    let lock = observed_lock_path();
    record_map_portfolio(&lock);
    let lock_arg = lock.to_str().expect("temp path should be valid UTF-8");

    let json_output = verify_command(
        "testdata/observed/portfolio-map-value-drift.json",
        "portfolio",
        lock_arg,
    )
    .args(["--format", "json"])
    .output()
    .expect("verify should run");
    assert_eq!(json_output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&json_output),
        json!({
            "version": 2,
            "command": "verify",
            "name": "portfolio",
            "provenance": "observed",
            "summary": {"breaking": 1},
            "changes": [{
                "kind": "incompatible_shape",
                "path": "$.by_broker.<map-value>.pnl_pct",
                "expected": "number",
                "actual": "string"
            }]
        })
    );

    let sarif_output = verify_command(
        "testdata/observed/portfolio-map-value-drift.json",
        "portfolio",
        lock_arg,
    )
    .args(["--format", "sarif"])
    .output()
    .expect("verify should run");
    assert_eq!(sarif_output.status.code(), Some(1));
    let sarif = parse_json_output(&sarif_output);
    assert_eq!(
        sarif["runs"][0]["results"][0]["ruleId"],
        "apiwatch/verify-observed-incompatible-shape"
    );
    assert_eq!(
        sarif["runs"][0]["results"][0]["message"]["text"],
        "incompatible shape at $.by_broker.<map-value>.pnl_pct: expected number, found string"
    );
    assert_eq!(
        sarif["runs"][0]["results"][0]["partialFingerprints"]["apiwatch/v1"],
        "verify-observed:portfolio:apiwatch/verify-observed-incompatible-shape:$.by_broker.<map-value>.pnl_pct:number:string"
    );
    assert!(!json_output
        .stdout
        .windows(b"verify-secret".len())
        .any(|part| part == b"verify-secret"));
    assert!(!json_output
        .stdout
        .windows(b"acme".len())
        .any(|part| part == b"acme"));
    assert!(!json_output
        .stdout
        .windows(b"globex".len())
        .any(|part| part == b"globex"));
    assert!(!sarif_output
        .stdout
        .windows(b"verify-secret".len())
        .any(|part| part == b"verify-secret"));
    assert!(!sarif_output
        .stdout
        .windows(b"acme".len())
        .any(|part| part == b"acme"));
    assert!(!sarif_output
        .stdout
        .windows(b"globex".len())
        .any(|part| part == b"globex"));

    fs::remove_file(lock).ok();
}

#[test]
fn verify_observed_json_body_with_matching_shape() {
    let lock = observed_lock_path();
    record_portfolio(&lock);
    let lock_arg = lock.to_str().expect("temp path should be valid UTF-8");

    verify_command(
        "testdata/observed/portfolio-matching.json",
        "portfolio",
        lock_arg,
    )
    .assert()
    .success()
    .stdout("Verified portfolio\n");

    fs::remove_file(lock).ok();
}

#[test]
fn verify_matching_observed_json_honors_json_format() {
    let lock = observed_lock_path();
    record_portfolio(&lock);
    let lock_arg = lock.to_str().expect("temp path should be valid UTF-8");

    let output = verify_command(
        "testdata/observed/portfolio-matching.json",
        "portfolio",
        lock_arg,
    )
    .args(["--format", "json"])
    .output()
    .expect("verify should run");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        parse_json_output(&output),
        json!({
            "version": 2,
            "command": "verify",
            "name": "portfolio",
            "provenance": "observed",
            "summary": {"breaking": 0},
            "changes": []
        })
    );

    fs::remove_file(lock).ok();
}

#[test]
fn verify_matching_observed_json_honors_sarif_format() {
    let lock = observed_lock_path();
    record_portfolio(&lock);
    let lock_arg = lock.to_str().expect("temp path should be valid UTF-8");

    let output = verify_command(
        "testdata/observed/portfolio-matching.json",
        "portfolio",
        lock_arg,
    )
    .args(["--format", "sarif"])
    .output()
    .expect("verify should run");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let rendered = parse_json_output(&output);
    assert_eq!(rendered["version"], "2.1.0");
    assert_eq!(
        rendered["runs"][0]["results"].as_array().map(Vec::len),
        Some(0)
    );
    assert_eq!(
        sarif_rule_ids(&rendered),
        vec![
            "apiwatch/verify-observed-missing-required-field",
            "apiwatch/verify-observed-incompatible-shape"
        ]
    );

    fs::remove_file(lock).ok();
}

#[test]
fn verify_observed_json_reports_a_missing_required_field_without_values() {
    let lock = observed_lock_path();
    record_portfolio(&lock);
    let lock_arg = lock.to_str().expect("temp path should be valid UTF-8");

    verify_command(
        "testdata/observed/portfolio-missing-required.json",
        "portfolio",
        lock_arg,
    )
    .assert()
    .code(1)
    .stdout("BREAKING $.summary.current_value: required field missing\n")
    .stderr(predicate::str::is_empty());

    fs::remove_file(lock).ok();
}

#[test]
fn verify_observed_json_reports_type_drift_without_values() {
    let lock = observed_lock_path();
    record_portfolio(&lock);
    let lock_arg = lock.to_str().expect("temp path should be valid UTF-8");

    verify_command(
        "testdata/observed/portfolio-type-drift.json",
        "portfolio",
        lock_arg,
    )
    .assert()
    .code(1)
    .stdout("BREAKING $.live_price: expected null | number, found string\n")
    .stdout(predicate::str::contains("recording-secret").not());

    fs::remove_file(lock).ok();
}

#[test]
fn verify_observed_json_format_reports_versioned_drift() {
    let lock = observed_lock_path();
    record_portfolio(&lock);
    let lock_arg = lock.to_str().expect("temp path should be valid UTF-8");

    let output = verify_command(
        "testdata/observed/portfolio-missing-required.json",
        "portfolio",
        lock_arg,
    )
    .args(["--format", "json"])
    .output()
    .expect("verify should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output),
        json!({
            "version": 2,
            "command": "verify",
            "name": "portfolio",
            "provenance": "observed",
            "summary": {"breaking": 1},
            "changes": [{
                "kind": "missing_required_field",
                "path": "$.summary.current_value"
            }]
        })
    );

    fs::remove_file(lock).ok();
}

#[test]
fn verify_observed_sarif_reports_a_lockfile_finding_without_values() {
    let lock = observed_lock_path();
    record_portfolio(&lock);
    let lock_arg = lock.to_str().expect("temp path should be valid UTF-8");

    let output = verify_command(
        "testdata/observed/portfolio-missing-required.json",
        "portfolio",
        lock_arg,
    )
    .args(["--format", "sarif"])
    .output()
    .expect("verify should run");

    assert_eq!(output.status.code(), Some(1));
    let rendered = parse_json_output(&output);
    assert_eq!(rendered["version"], "2.1.0");
    assert_eq!(
        rendered["runs"][0]["results"][0]["ruleId"],
        "apiwatch/verify-observed-missing-required-field"
    );
    assert_eq!(
        rendered["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
            ["uri"],
        lock_arg.replace(':', "%3A").replace('\\', "/")
    );
    assert!(!output
        .stdout
        .windows(b"recording-secret".len())
        .any(|part| part == b"recording-secret"));

    fs::remove_file(lock).ok();
}

#[test]
fn verify_sarif_reports_drift_and_exit_one() {
    let output = verify_command(
        "testdata/openapi/verify_current.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .args(["--format", "sarif"])
    .output()
    .expect("Verify command should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));
    let rendered = parse_json_output(&output);
    assert_eq!(
        sarif_rule_ids(&rendered),
        vec![
            "apiwatch/diff-breaking",
            "apiwatch/diff-warning",
            "apiwatch/diff-non-breaking",
            "apiwatch/verify-removed",
            "apiwatch/verify-added",
        ]
    );
    let results = rendered["runs"][0]["results"]
        .as_array()
        .expect("SARIF results should be an array");
    assert_eq!(
        results
            .iter()
            .map(|result| result["ruleId"]
                .as_str()
                .expect("SARIF result should have a rule ID"))
            .collect::<Vec<_>>(),
        vec![
            "apiwatch/diff-breaking",
            "apiwatch/diff-breaking",
            "apiwatch/diff-warning",
            "apiwatch/diff-warning",
        ]
    );
    assert_eq!(results[0]["level"], "error");
    assert_eq!(results[0]["message"]["text"], "endpoint removed");
    assert_eq!(
        results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "testdata/lock/verify_users.lock"
    );
    assert_eq!(
        results[0]["partialFingerprints"]["apiwatch/v1"],
        "verify:users:apiwatch/diff-breaking:GET:/users:endpoint removed"
    );
    assert_eq!(
        results[2]["message"]["text"],
        "endpoint added outside route-only lock"
    );
    assert_eq!(results[2]["level"], "warning");
    assert_eq!(
        results[2]["partialFingerprints"]["apiwatch/v1"],
        "verify:users:apiwatch/diff-warning:POST:/users:endpoint added outside route-only lock"
    );
}

#[test]
fn verify_sarif_reports_matching_contract_and_exit_zero() {
    let output = verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .args(["--format", "sarif"])
    .output()
    .expect("Verify command should run");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let rendered = parse_json_output(&output);
    assert_eq!(
        sarif_rule_ids(&rendered),
        vec![
            "apiwatch/diff-breaking",
            "apiwatch/diff-warning",
            "apiwatch/diff-non-breaking",
            "apiwatch/verify-removed",
            "apiwatch/verify-added",
        ]
    );
    assert_eq!(rendered["runs"][0]["results"], json!([]));
}

#[test]
fn verify_sarif_keeps_invalid_format_rejection() {
    let output = verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .args(["--format", "yaml"])
    .output()
    .expect("Verify command should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid value 'yaml' for '--format <FORMAT>'"));
}

#[test]
fn verify_json_reports_drift_and_exit_one() {
    let output = verify_command(
        "testdata/openapi/verify_current.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .args(["--format", "json"])
    .output()
    .expect("Verify command should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));
    let rendered: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        rendered,
        json!({
            "version": 2,
            "command": "verify",
            "name": "users",
            "provenance": "declared",
            "coverage": "routes",
            "limitations": [{
                "code": "route_only_lock",
                "message": "api.lock v1/v2 declared entry is route-only; full contract changes are not verified"
            }],
            "summary": { "breaking": 2, "warning": 2, "non_breaking": 0 },
            "changes": [
                { "severity": "breaking", "method": "GET", "path": "/users", "message": "endpoint removed" },
                { "severity": "breaking", "method": "GET", "path": "/zeta", "message": "endpoint removed" },
                { "severity": "warning", "method": "POST", "path": "/users", "message": "endpoint added outside route-only lock" },
                { "severity": "warning", "method": "POST", "path": "/zeta", "message": "endpoint added outside route-only lock" }
            ]
        })
    );
}

#[test]
fn verify_json_reports_matching_contract_and_exit_zero() {
    let output = verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .args(["--format", "json"])
    .output()
    .expect("Verify command should run");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let rendered: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(rendered["name"], "users");
    assert_eq!(
        rendered["summary"],
        json!({ "breaking": 0, "warning": 0, "non_breaking": 0 })
    );
    assert_eq!(rendered["changes"], json!([]));
}

#[test]
fn verify_default_format_preserves_text_output() {
    verify_command(
        "testdata/openapi/verify_current.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(1)
    .stdout(
        "Breaking changes\n- GET /users: endpoint removed\n- GET /zeta: endpoint removed\n\nWarnings\n- POST /users: endpoint added outside route-only lock\n- POST /zeta: endpoint added outside route-only lock\n",
    );
}

#[test]
fn verify_rejects_invalid_format() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .args(["--format", "yaml"])
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains(
        "invalid value 'yaml' for '--format <FORMAT>'",
    ));
}

#[test]
fn verify_exits_zero_for_matching_remote_operations() {
    let url = serve_once(
        "200 OK",
        "application/yaml",
        include_str!("../testdata/openapi/verify_matching.yaml"),
        "openapi.yaml",
    );
    verify_command(&url, "users", "testdata/lock/verify_users.lock")
        .assert()
        .success()
        .stdout("Verified users\n");
}

#[test]
fn verify_ignores_http_proxy_configuration() {
    let proxy = serve_proxy_probe(include_str!("../testdata/openapi/verify_matching.yaml"));
    let mut command = verify_command(
        "http://apiwatch-proxy-test.invalid/openapi.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    );
    command.env_clear().env("HTTP_PROXY", &proxy.url);

    command
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(
            "failed to request remote OpenAPI document",
        ));
    proxy.assert_not_used();
}

#[test]
fn verify_exits_one_for_remote_operation_drift() {
    let url = serve_once(
        "200 OK",
        "application/yaml",
        include_str!("../testdata/openapi/verify_current.yaml"),
        "openapi.yaml",
    );
    verify_command(&url, "users", "testdata/lock/verify_users.lock")
        .assert()
        .code(1)
        .stdout(
            "Breaking changes\n- GET /users: endpoint removed\n- GET /zeta: endpoint removed\n\nWarnings\n- POST /users: endpoint added outside route-only lock\n- POST /zeta: endpoint added outside route-only lock\n",
        );
}

#[test]
fn verify_exits_two_for_a_remote_non_success_status() {
    let url = serve_once(
        "503 Service Unavailable",
        "text/plain",
        "unavailable",
        "openapi.yaml",
    );
    verify_command(&url, "users", "testdata/lock/verify_users.lock")
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(
            "remote OpenAPI request returned a non-success status",
        ));
}

#[test]
fn verify_exits_two_for_an_unsupported_remote_url_scheme() {
    verify_command(
        "ftp://example.test/openapi.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("unsupported OpenAPI URL scheme"));
}

#[test]
fn verify_exits_zero_for_matching_remote_json_operations() {
    let url = serve_once(
        "200 OK",
        "application/json",
        include_str!("../testdata/openapi/verify_matching.json"),
        "openapi.yaml",
    );
    verify_command(&url, "users", "testdata/lock/verify_users.lock")
        .assert()
        .success()
        .stdout("Verified users\n");
}

#[test]
fn verify_exits_zero_for_matching_locked_operations() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .success()
    .stdout("Verified users\n");
}

#[test]
fn verify_exits_zero_with_warning_for_an_added_operation() {
    verify_command(
        "testdata/openapi/verify_added.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .success()
    .stdout("Warnings\n- POST /users: endpoint added outside route-only lock\n");
}

#[test]
fn verify_exits_one_for_a_removed_operation() {
    verify_command(
        "testdata/openapi/verify_removed.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(1)
    .stdout("Breaking changes\n- GET /users: endpoint removed\n");
}

#[test]
fn verify_renders_removed_operations_before_added_operations() {
    verify_command(
        "testdata/openapi/verify_current.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(1)
    .stdout(
        "\
Breaking changes
- GET /users: endpoint removed
- GET /zeta: endpoint removed

Warnings
- POST /users: endpoint added outside route-only lock
- POST /zeta: endpoint added outside route-only lock
",
    );
}

#[test]
fn verify_orders_operations_by_method_and_path_within_each_group() {
    verify_command(
        "testdata/openapi/verify_ordering.yaml",
        "users",
        "testdata/lock/verify_ordering.lock",
    )
    .assert()
    .code(1)
    .stdout(
        "\
Breaking changes
- GET /beta: endpoint removed
- GET /zeta: endpoint removed
- POST /zeta: endpoint removed

Warnings
- GET /alpha: endpoint added outside route-only lock
- GET /omega: endpoint added outside route-only lock
- PUT /zeta: endpoint added outside route-only lock
",
    );
}

#[test]
fn verify_exits_two_for_a_whitespace_only_api_name() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "   ",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("api name cannot be empty"));
}

#[test]
fn verify_exits_two_for_a_missing_api_name() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "payments",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains(
        "api payments not found in lockfile",
    ));
}

#[test]
fn verify_exits_two_for_invalid_lockfile_yaml() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_invalid_yaml.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("failed to parse api.lock YAML"));
}

#[test]
fn verify_exits_two_for_an_invalid_locked_operation_method() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_invalid_operation_method.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains(
        "unsupported locked operation method",
    ));
}

#[test]
fn verify_exits_two_for_an_invalid_locked_operation_path() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_invalid_operation_path.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains(
        "locked operation path contains a control character",
    ));
}

#[test]
fn verify_rejects_openapi_31_with_an_accurate_message() {
    verify_command(
        "testdata/openapi/unsupported_31.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("OpenAPI 3.1 is not yet supported"));
}

#[test]
fn verify_exits_two_for_an_openapi_path_with_a_control_character() {
    verify_command(
        "testdata/openapi/verify_invalid_operation_path.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains(
        "OpenAPI path contains a control character",
    ));
}

#[test]
fn verify_exits_two_for_an_empty_openapi_path() {
    verify_command(
        "testdata/openapi/verify_empty_path.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("OpenAPI path cannot be empty"));
}

#[test]
fn verify_exits_two_for_an_openapi_path_without_a_leading_slash() {
    verify_command(
        "testdata/openapi/verify_non_slash_path.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("OpenAPI path must start with /"));
}

#[test]
fn verify_accepts_openapi_path_extensions() {
    verify_command(
        "testdata/openapi/verify_with_path_extension.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .success()
    .stdout("Verified users\n");
}

#[test]
fn verify_exits_two_for_a_non_slash_json_openapi_path() {
    verify_command(
        "testdata/openapi/verify_non_slash_path.json",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("OpenAPI path must start with /"));
}

#[test]
fn verify_exits_two_for_a_lockfile_source_with_a_control_character() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_invalid_source.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains(
        "api.lock source contains a control character",
    ));
}

#[test]
fn verify_exits_two_for_invalid_openapi_input() {
    verify_command(
        "testdata/openapi/invalid_yaml.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("failed to parse OpenAPI YAML"));
}

#[test]
fn verify_exits_two_for_an_invalid_v4_lockfile() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_unsupported_version.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("failed to parse api.lock v4 YAML"));
}

#[test]
fn verify_exits_two_for_an_unsupported_lockfile_source() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_unsupported_source.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains(
        "unsupported api.lock source remote",
    ));
}
