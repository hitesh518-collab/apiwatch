use assert_cmd::Command;
use predicates::prelude::*;

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
            let mut byte = [0_u8; 1];
            stream
                .read_exact(&mut byte)
                .expect("test server should read request headers");
            request.push(byte[0]);
            assert!(
                request.len() <= 8 * 1024,
                "test server request headers exceed 8 KiB"
            );
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

fn verify_command(openapi: &str, name: &str, lock: &str) -> Command {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");
    command.args(["verify", openapi, "--name", name, "--lock", lock]);
    command
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
