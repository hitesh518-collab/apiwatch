use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn diff_exits_one_for_breaking_change() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/endpoint_removed_old.yaml",
            "testdata/openapi/endpoint_removed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains("GET /users: endpoint removed"));
}

#[test]
fn diff_exits_zero_for_non_breaking_change() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/no_breaking_old.yaml",
            "testdata/openapi/no_breaking_new.yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Non-breaking changes"))
        .stdout(predicate::str::contains("GET /teams: endpoint added"));
}

#[test]
fn diff_exits_two_for_unsupported_openapi_version() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/unsupported_version.yaml",
            "testdata/openapi/no_breaking_new.yaml",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "unsupported OpenAPI version 2.0.0",
        ));
}

#[test]
fn diff_exits_one_for_removed_response_field() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/response_field_removed_old.yaml",
            "testdata/openapi/response_field_removed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: response 200 application/json field name removed",
        ));
}

#[test]
fn diff_exits_zero_for_added_response_field() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/response_field_removed_new.yaml",
            "testdata/openapi/response_field_removed_old.yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Non-breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: response 200 application/json field name added",
        ));
}

#[test]
fn diff_exits_one_for_response_field_type_change() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/response_type_changed_old.yaml",
            "testdata/openapi/response_type_changed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: response 200 application/json field id type changed from string to integer",
        ));
}

#[test]
fn diff_exits_one_when_response_field_becomes_nullable() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/response_nullable_changed_old.yaml",
            "testdata/openapi/response_nullable_changed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: response 200 application/json field email nullable changed from false to true",
        ));
}

#[test]
fn diff_exits_one_when_response_enum_value_is_added() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/response_enum_changed_old.yaml",
            "testdata/openapi/response_enum_changed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: response 200 application/json field status enum value pending added",
        ));
}

#[test]
fn diff_exits_one_for_removed_nested_response_field() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/response_nested_field_removed_old.yaml",
            "testdata/openapi/response_nested_field_removed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: response 200 application/json field profile.displayName removed",
        ));
}

#[test]
fn diff_exits_one_for_added_required_request_field() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/request_required_field_added_old.yaml",
            "testdata/openapi/request_required_field_added_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "POST /users: request application/json field email added as required",
        ));
}

#[test]
fn diff_exits_zero_for_added_optional_request_field() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/request_optional_field_added_old.yaml",
            "testdata/openapi/request_optional_field_added_new.yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Non-breaking changes"))
        .stdout(predicate::str::contains(
            "POST /users: request application/json field nickname added as optional",
        ));
}

#[test]
fn diff_exits_one_for_removed_request_field() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/request_field_removed_old.yaml",
            "testdata/openapi/request_field_removed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "POST /users: request application/json field nickname removed",
        ));
}

#[test]
fn diff_exits_one_when_request_field_becomes_required() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/request_field_became_required_old.yaml",
            "testdata/openapi/request_field_became_required_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "POST /users: request application/json field email changed from optional to required",
        ));
}

#[test]
fn diff_exits_one_when_request_enum_value_is_removed() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/request_enum_value_removed_old.yaml",
            "testdata/openapi/request_enum_value_removed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "POST /users: request application/json field status enum value inactive removed",
        ));
}

#[test]
fn diff_exits_one_for_request_field_type_change() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/request_type_changed_old.yaml",
            "testdata/openapi/request_type_changed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "POST /users: request application/json field age type changed from integer to string",
        ));
}

#[test]
fn diff_exits_one_when_request_field_becomes_non_nullable() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/request_nullable_narrowed_old.yaml",
            "testdata/openapi/request_nullable_narrowed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "POST /users: request application/json field email nullable changed from true to false",
        ));
}
