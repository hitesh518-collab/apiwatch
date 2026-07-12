use assert_cmd::Command;
use predicates::prelude::*;

fn verify_command(openapi: &str, name: &str, lock: &str) -> Command {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");
    command.args(["verify", openapi, "--name", name, "--lock", lock]);
    command
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
