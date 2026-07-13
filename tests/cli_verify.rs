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
            "apiwatch/verify-removed",
            "apiwatch/verify-removed",
            "apiwatch/verify-added",
            "apiwatch/verify-added",
        ]
    );
    assert_eq!(results[0]["level"], "error");
    assert_eq!(
        results[0]["message"]["text"],
        "locked operation removed: GET /users"
    );
    assert_eq!(
        results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "testdata/lock/verify_users.lock"
    );
    assert_eq!(
        results[0]["partialFingerprints"]["apiwatch/v1"],
        "verify:users:apiwatch/verify-removed:GET:/users"
    );
    assert_eq!(
        results[2]["message"]["text"],
        "unlocked operation added: POST /users"
    );
    assert_eq!(results[2]["level"], "warning");
    assert_eq!(
        results[2]["partialFingerprints"]["apiwatch/v1"],
        "verify:users:apiwatch/verify-added:POST:/users"
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
            "version": 1,
            "command": "verify",
            "name": "users",
            "summary": { "removed": 2, "added": 2 },
            "changes": [
                { "kind": "removed", "method": "GET", "path": "/users" },
                { "kind": "removed", "method": "GET", "path": "/zeta" },
                { "kind": "added", "method": "POST", "path": "/users" },
                { "kind": "added", "method": "POST", "path": "/zeta" }
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
    assert_eq!(rendered["summary"], json!({ "removed": 0, "added": 0 }));
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
    .stdout("REMOVED GET /users\nREMOVED GET /zeta\nADDED POST /users\nADDED POST /zeta\n");
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
        .stdout("REMOVED GET /users\nREMOVED GET /zeta\nADDED POST /users\nADDED POST /zeta\n");
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
fn verify_exits_one_for_an_added_operation() {
    verify_command(
        "testdata/openapi/verify_added.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(1)
    .stdout("ADDED POST /users\n");
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
    .stdout("REMOVED GET /users\n");
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
REMOVED GET /users
REMOVED GET /zeta
ADDED POST /users
ADDED POST /zeta
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
REMOVED GET /beta
REMOVED GET /zeta
REMOVED POST /zeta
ADDED GET /alpha
ADDED GET /omega
ADDED PUT /zeta
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
fn verify_exits_two_for_an_unsupported_lockfile_version() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_unsupported_version.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("unsupported api.lock version 2"));
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
