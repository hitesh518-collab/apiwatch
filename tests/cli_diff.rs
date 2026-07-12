use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};

#[test]
fn diff_json_reports_breaking_changes_and_exit_one() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/endpoint_removed_old.yaml",
            "testdata/openapi/endpoint_removed_new.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));
    let rendered: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        rendered,
        json!({
            "version": 1,
            "command": "diff",
            "summary": { "breaking": 1, "warning": 0, "non_breaking": 0 },
            "changes": [{
                "severity": "breaking",
                "method": "GET",
                "path": "/users",
                "message": "endpoint removed"
            }]
        })
    );
}

#[test]
fn diff_json_reports_warning_only_change_and_exit_zero() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/status_error_added_old.yaml",
            "testdata/openapi/status_error_added_new.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let rendered: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        rendered,
        json!({
            "version": 1,
            "command": "diff",
            "summary": { "breaking": 0, "warning": 1, "non_breaking": 0 },
            "changes": [{
                "severity": "warning",
                "method": "GET",
                "path": "/users",
                "message": "response status 429 added"
            }]
        })
    );
}

#[test]
fn diff_json_reports_no_changes() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/no_breaking_old.yaml",
            "testdata/openapi/no_breaking_old.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let rendered: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        rendered,
        json!({
            "version": 1,
            "command": "diff",
            "summary": { "breaking": 0, "warning": 0, "non_breaking": 0 },
            "changes": []
        })
    );
}

#[test]
fn diff_defaults_to_byte_compatible_text_output() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/endpoint_removed_old.yaml",
            "testdata/openapi/endpoint_removed_new.yaml",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        output.stdout,
        b"Breaking changes\n- GET /users: endpoint removed\n"
    );
}

#[test]
fn diff_rejects_invalid_output_format() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/no_breaking_old.yaml",
            "testdata/openapi/no_breaking_old.yaml",
            "--format",
            "yaml",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid value 'yaml' for '--format <FORMAT>'"));
}

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
fn diff_exits_two_for_invalid_yaml_spec() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/invalid_yaml.yaml",
            "testdata/openapi/no_breaking_new.yaml",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("failed to parse OpenAPI YAML"));
}

#[test]
fn diff_exits_two_for_invalid_json_spec() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/invalid_json.json",
            "testdata/openapi/no_breaking_new.yaml",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("failed to parse OpenAPI JSON"));
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
fn diff_exits_one_for_removed_response_array_item_field() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/response_array_item_field_removed_old.yaml",
            "testdata/openapi/response_array_item_field_removed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: response 200 application/json field items.name removed",
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
fn diff_exits_one_for_added_required_request_array_item_field() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/request_array_required_item_field_added_old.yaml",
            "testdata/openapi/request_array_required_item_field_added_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "POST /users: request application/json field items.email added as required",
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

#[test]
fn diff_exits_one_for_added_required_query_parameter() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/parameter_required_query_added_old.yaml",
            "testdata/openapi/parameter_required_query_added_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: query parameter limit added as required",
        ));
}

#[test]
fn diff_exits_zero_for_added_optional_query_parameter() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/parameter_optional_query_added_old.yaml",
            "testdata/openapi/parameter_optional_query_added_new.yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Non-breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: query parameter cursor added as optional",
        ));
}

#[test]
fn diff_exits_one_for_removed_query_parameter() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/parameter_query_removed_old.yaml",
            "testdata/openapi/parameter_query_removed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: query parameter cursor removed",
        ));
}

#[test]
fn diff_exits_one_for_query_parameter_type_change() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/parameter_query_type_changed_old.yaml",
            "testdata/openapi/parameter_query_type_changed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: query parameter limit schema type changed from integer to string",
        ));
}

#[test]
fn diff_exits_one_for_path_parameter_type_change() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/parameter_path_type_changed_old.yaml",
            "testdata/openapi/parameter_path_type_changed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users/{userId}: path parameter userId schema type changed from string to integer",
        ));
}

#[test]
fn diff_exits_one_for_added_required_path_level_header_parameter() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/parameter_path_level_header_added_old.yaml",
            "testdata/openapi/parameter_path_level_header_added_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: header parameter X-Tenant-Id added as required",
        ));
}

#[test]
fn diff_exits_one_for_removed_cookie_parameter() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/parameter_cookie_removed_old.yaml",
            "testdata/openapi/parameter_cookie_removed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: cookie parameter session removed",
        ));
}

#[test]
fn diff_exits_one_when_query_parameter_becomes_required() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/parameter_query_became_required_old.yaml",
            "testdata/openapi/parameter_query_became_required_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: query parameter cursor changed from optional to required",
        ));
}

#[test]
fn diff_exits_one_for_added_bearer_authentication() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/auth_bearer_added_old.yaml",
            "testdata/openapi/auth_bearer_added_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: authentication bearerAuth (bearer) added",
        ));
}

#[test]
fn diff_exits_one_for_added_api_key_authentication_from_global_security() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/auth_api_key_added_old.yaml",
            "testdata/openapi/auth_api_key_added_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: authentication apiKeyAuth (apiKey) added",
        ));
}

#[test]
fn diff_exits_one_for_added_basic_authentication() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/auth_basic_added_old.yaml",
            "testdata/openapi/auth_basic_added_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: authentication basicAuth (basic) added",
        ));
}

#[test]
fn diff_exits_one_for_added_oauth2_authentication() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/auth_oauth2_added_old.yaml",
            "testdata/openapi/auth_oauth2_added_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: authentication oauthAuth (oauth2) added",
        ));
}

#[test]
fn diff_exits_zero_for_removed_authentication() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/auth_bearer_added_new.yaml",
            "testdata/openapi/auth_bearer_added_old.yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Non-breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: authentication bearerAuth (bearer) removed",
        ));
}

#[test]
fn diff_exits_one_for_removed_success_status_code() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/status_success_removed_old.yaml",
            "testdata/openapi/status_success_removed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: response status 200 removed",
        ));
}

#[test]
fn diff_warns_for_added_error_status_code() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/status_error_added_old.yaml",
            "testdata/openapi/status_error_added_new.yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Warnings"))
        .stdout(predicate::str::contains(
            "GET /users: response status 429 added",
        ));
}

#[test]
fn diff_exits_zero_for_added_success_status_code() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/status_success_added_old.yaml",
            "testdata/openapi/status_success_added_new.yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Non-breaking changes"))
        .stdout(predicate::str::contains(
            "POST /users: response status 200 added",
        ));
}

#[test]
fn diff_resolves_component_schema_refs_for_response_diff() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/ref_response_schema_old.yaml",
            "testdata/openapi/ref_response_schema_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: response 200 application/json field name removed",
        ));
}

#[test]
fn diff_exits_two_for_circular_schema_ref() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/ref_circular_schema.yaml",
            "testdata/openapi/ref_response_schema_new.yaml",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "circular schema reference detected",
        ));
}

#[test]
fn diff_detects_oneof_branch_type_change() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/composition_oneof_changed_old.yaml",
            "testdata/openapi/composition_oneof_changed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /search: response 200 application/json field oneOf[0] type changed from string to integer",
        ));
}

#[test]
fn diff_detects_allof_branch_field_removed() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/composition_allof_changed_old.yaml",
            "testdata/openapi/composition_allof_changed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: response 200 application/json field allOf[0].name removed",
        ));
}

#[test]
fn diff_detects_anyof_branch_field_type_change() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/composition_anyof_changed_old.yaml",
            "testdata/openapi/composition_anyof_changed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /search: response 200 application/json field anyOf[0].result type changed from string to integer",
        ));
}

#[test]
fn diff_resolves_component_response_refs_for_response_diff() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/ref_component_response_old.yaml",
            "testdata/openapi/ref_component_response_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: response 200 application/json field name removed",
        ));
}

#[test]
fn diff_exits_two_for_circular_response_ref() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/ref_circular_response.yaml",
            "testdata/openapi/ref_component_response_new.yaml",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "circular response reference detected",
        ));
}

#[test]
fn diff_resolves_component_request_body_refs_for_request_diff() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/ref_component_request_body_old.yaml",
            "testdata/openapi/ref_component_request_body_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "POST /users: request application/json field email added as required",
        ));
}

#[test]
fn diff_exits_two_for_circular_request_body_ref() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/ref_circular_request_body.yaml",
            "testdata/openapi/ref_component_request_body_new.yaml",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "circular request body reference detected",
        ));
}

#[test]
fn diff_resolves_component_parameter_refs_for_parameter_diff() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/ref_component_parameter_old.yaml",
            "testdata/openapi/ref_component_parameter_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: query parameter limit added as required",
        ));
}

#[test]
fn diff_exits_two_for_circular_parameter_ref() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/ref_circular_parameter.yaml",
            "testdata/openapi/ref_component_parameter_new.yaml",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "circular parameter reference detected",
        ));
}

#[test]
fn diff_resolves_component_security_scheme_refs_for_auth_diff() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/auth_bearer_added_old.yaml",
            "testdata/openapi/ref_component_security_scheme_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: authentication bearerAuth (bearer) added",
        ));
}

#[test]
fn diff_exits_two_for_circular_security_scheme_ref() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/ref_circular_security_scheme.yaml",
            "testdata/openapi/ref_component_security_scheme_new.yaml",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "circular security scheme reference detected",
        ));
}

#[test]
fn diff_resolves_path_item_refs_for_parameter_diff() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/parameter_required_query_added_old.yaml",
            "testdata/openapi/ref_path_item_parameter_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains(
            "GET /users: query parameter limit added as required",
        ));
}

#[test]
fn diff_exits_two_for_circular_path_item_ref() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/ref_circular_path_item.yaml",
            "testdata/openapi/ref_path_item_parameter_new.yaml",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "circular path item reference detected",
        ));
}
