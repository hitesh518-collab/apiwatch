use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};

const PHASE2_D07_COLLIDING_PATHS: &str = "openapi: 3.0.3\ninfo: { title: D-07 collision, version: '1' }\npaths:\n  /users/{id}:\n    get:\n      parameters:\n        - { name: id, in: path, required: true, schema: { type: string } }\n      responses: { '200': { description: ok } }\n  /users/{name}:\n    get:\n      parameters:\n        - { name: name, in: path, required: true, schema: { type: string } }\n      responses: { '200': { description: ok } }\n";

const PHASE2_D10_ITEM_DIRECTION_OLD: &str = "openapi: 3.0.3\ninfo: { title: D-10 directions, version: '1' }\npaths:\n  /request-add:\n    post:\n      requestBody: { content: { application/json: { schema: { type: array } } } }\n      responses: { '200': { description: OK } }\n  /request-remove:\n    post:\n      requestBody: { content: { application/json: { schema: { type: array, items: { type: string } } } } }\n      responses: { '200': { description: OK } }\n  /response-add:\n    get:\n      responses: { '200': { description: OK, content: { application/json: { schema: { type: array } } } } }\n  /response-remove:\n    get:\n      responses: { '200': { description: OK, content: { application/json: { schema: { type: array, items: { type: string } } } } } }\n";
const PHASE2_D10_ITEM_DIRECTION_NEW: &str = "openapi: 3.0.3\ninfo: { title: D-10 directions, version: '2' }\npaths:\n  /request-add:\n    post:\n      requestBody: { content: { application/json: { schema: { type: array, items: { type: string } } } } }\n      responses: { '200': { description: OK } }\n  /request-remove:\n    post:\n      requestBody: { content: { application/json: { schema: { type: array } } } }\n      responses: { '200': { description: OK } }\n  /response-add:\n    get:\n      responses: { '200': { description: OK, content: { application/json: { schema: { type: array, items: { type: string } } } } } }\n  /response-remove:\n    get:\n      responses: { '200': { description: OK, content: { application/json: { schema: { type: array } } } } }\n";

#[test]
fn phase2_d10_classifies_array_item_presence_directionally() {
    let old = tempfile::NamedTempFile::new().expect("old document should be created");
    let new = tempfile::NamedTempFile::new().expect("new document should be created");
    std::fs::write(old.path(), PHASE2_D10_ITEM_DIRECTION_OLD).expect("old document should write");
    std::fs::write(new.path(), PHASE2_D10_ITEM_DIRECTION_NEW).expect("new document should write");

    let output = Command::cargo_bin("apiwatch")
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

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"],
        json!([
            { "severity": "non_breaking", "method": "GET", "path": "/response-add", "message": "response 200 application/json field items added" },
            { "severity": "breaking", "method": "GET", "path": "/response-remove", "message": "response 200 application/json field items removed" },
            { "severity": "breaking", "method": "POST", "path": "/request-add", "message": "request application/json field items added as required" },
            { "severity": "non_breaking", "method": "POST", "path": "/request-remove", "message": "request application/json field items removed" },
        ])
    );
}

#[test]
fn phase2_d10_compares_first_class_array_items_directionally() {
    let output = Command::cargo_bin("apiwatch")
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

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"],
        json!([
            { "severity": "warning", "method": "GET", "path": "/nested", "message": "response 200 application/json field items.items format changed from uuid to date-time" },
            { "severity": "breaking", "method": "GET", "path": "/users", "message": "response 200 application/json field items.name removed" },
            { "severity": "breaking", "method": "POST", "path": "/users", "message": "request application/json field items.email added as required" }
        ])
    );
}

#[test]
fn phase2_d08_matches_authentication_by_wire_identity() {
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/phase2_d08_auth_identity_old.yaml",
            "testdata/openapi/phase2_d08_auth_identity_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "GET /keyed: authentication apiKeyAuth changed identity",
        ))
        .stdout(predicate::str::contains("GET /renamed").not());
}

#[test]
fn phase2_d08_rejects_duplicate_authentication_identity_without_echoing_controls() {
    let document = tempfile::Builder::new()
        .suffix(".yaml")
        .tempfile()
        .expect("temporary OpenAPI document should be created");
    std::fs::write(
        document.path(),
        "openapi: 3.0.3\ninfo: { title: D-08 duplicate, version: '1' }\ncomponents:\n  securitySchemes:\n    bearerAuth:\n      type: http\n      scheme: bearer\n    \"duplicate\\e\":\n      type: http\n      scheme: bearer\npaths:\n  /duplicate:\n    get:\n      security:\n        - bearerAuth: []\n          \"duplicate\\e\": []\n      responses: { '200': { description: OK } }\n",
    )
    .expect("temporary OpenAPI document should be written");

    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            document
                .path()
                .to_str()
                .expect("temporary path should be UTF-8"),
            "testdata/openapi/phase2_d08_auth_identity_old.yaml",
        ])
        .output()
        .expect("diff command should run");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("duplicate authentication identity"),
        "{stderr}"
    );
    assert!(!stderr.contains('\u{1b}'), "{stderr:?}");
}

#[test]
fn phase2_d07_ignores_positional_path_placeholder_renames() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/phase2_d07_path_template_old.yaml",
            "testdata/openapi/phase2_d07_path_template_new.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("diff command should run");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(parse_json_output(&output)["changes"], json!([]));
}

#[test]
fn phase2_d07_rejects_ambiguous_positional_path_template_identity() {
    let document = tempfile::Builder::new()
        .suffix(".yaml")
        .tempfile()
        .expect("temporary OpenAPI document should be created");
    std::fs::write(document.path(), PHASE2_D07_COLLIDING_PATHS)
        .expect("temporary OpenAPI document should be written");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            document
                .path()
                .to_str()
                .expect("temporary path should be UTF-8"),
            "testdata/openapi/phase2_d07_path_template_old.yaml",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "ambiguous operation identity GET /users/{0}",
        ));
}

#[test]
fn phase2_d07_rejects_duplicate_same_layer_parameters() {
    let document = tempfile::Builder::new()
        .suffix(".yaml")
        .tempfile()
        .expect("temporary OpenAPI document should be created");
    std::fs::write(
        document.path(),
        "openapi: 3.0.3\ninfo: { title: D-07 duplicate parameters, version: '1' }\npaths:\n  /users:\n    get:\n      parameters:\n        - { name: page, in: query, schema: { type: integer } }\n        - { name: page, in: query, schema: { type: integer } }\n      responses: { '200': { description: ok } }\n",
    )
    .expect("temporary OpenAPI document should be written");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            document
                .path()
                .to_str()
                .expect("temporary path should be UTF-8"),
            "testdata/openapi/phase2_d07_path_template_old.yaml",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("duplicate parameter query:page"));
}

fn phase2_d06_openapi(server: &str) -> String {
    format!(
        "openapi: 3.0.3\ninfo: {{ title: D-06 Regression, version: '1' }}\nservers:\n  - url: {server:?}\npaths:\n  /users:\n    get:\n      responses: {{ '204': {{ description: ok }} }}\n"
    )
}

fn phase2_d06_source(server: &str) -> tempfile::NamedTempFile {
    let source = tempfile::Builder::new()
        .suffix(".yaml")
        .tempfile()
        .expect("temporary source should be created");
    std::fs::write(source.path(), phase2_d06_openapi(server))
        .expect("temporary source should be written");
    source
}

#[test]
fn phase2_d06_redacts_entire_query_values_that_mix_variables_and_literals() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = phase2_d06_source(
        "https://api.example.com/v1?tenant={tenant}-literal-secret&token=plain-secret",
    );
    let lock = directory.path().join("private.lock");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            source
                .path()
                .to_str()
                .expect("temporary path should be UTF-8"),
            "--name",
            "private",
            "--output",
            lock.to_str().expect("temporary path should be UTF-8"),
        ])
        .assert()
        .success();

    let rendered = std::fs::read_to_string(&lock).expect("lock should be readable");
    assert!(rendered.contains("tenant={tenant}{redacted}"));
    assert!(rendered.contains("token={redacted}"));
    assert!(!rendered.contains("literal-secret"));
    assert!(!rendered.contains("plain-secret"));

    let changed = phase2_d06_source(
        "https://api.example.com/v1?tenant={tenant}-other-secret&token=other-secret",
    );
    let diff = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            source
                .path()
                .to_str()
                .expect("temporary path should be UTF-8"),
            changed
                .path()
                .to_str()
                .expect("temporary path should be UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");
    assert_eq!(diff.status.code(), Some(0));
    let findings = String::from_utf8_lossy(&diff.stdout);
    for secret in ["literal-secret", "plain-secret", "other-secret"] {
        assert!(!findings.contains(secret), "finding leaked {secret}");
    }
}

#[test]
fn phase2_d06_treats_percent_encoded_braces_as_literal_query_data() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let old = phase2_d06_source("https://api.example.com/v1?token=%7Bliteral-secret%7D");
    let new = phase2_d06_source("https://api.example.com/v1?token=%7Bcounterpart-secret%7D");
    let lock = directory.path().join("encoded-braces.lock");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            old.path().to_str().expect("temporary path should be UTF-8"),
            "--name",
            "encoded-braces",
            "--output",
            lock.to_str().expect("temporary path should be UTF-8"),
        ])
        .assert()
        .success();
    let lock_bytes = std::fs::read(&lock).expect("lock should be readable");
    let rendered_lock = String::from_utf8_lossy(&lock_bytes);
    assert!(rendered_lock.contains("token={redacted}"));

    let diff = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            old.path().to_str().expect("temporary path should be UTF-8"),
            new.path().to_str().expect("temporary path should be UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");
    assert_eq!(diff.status.code(), Some(0));
    let findings = String::from_utf8_lossy(&diff.stdout);
    for secret in ["literal-secret", "counterpart-secret"] {
        assert!(!rendered_lock.contains(secret), "lock leaked {secret}");
        assert!(!findings.contains(secret), "finding leaked {secret}");
    }
}

#[test]
fn phase2_d06_keeps_query_placeholder_identity_while_redacting_literal_portions() {
    let old = phase2_d06_source("https://api.example.com/v1?tenant={tenant}");
    let new = phase2_d06_source("https://api.example.com/v1?tenant={organization}");
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            old.path().to_str().expect("temporary path should be UTF-8"),
            new.path().to_str().expect("temporary path should be UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"],
        json!([
            {"severity": "breaking", "method": "GET", "path": "/users", "message": "server https://api.example.com/v1?tenant={tenant} removed"},
            {"severity": "non_breaking", "method": "GET", "path": "/users", "message": "server https://api.example.com/v1?tenant={organization} added"}
        ])
    );
}

#[test]
fn phase2_d06_does_not_restore_placeholders_into_percent_decoded_query_keys() {
    let old = phase2_d06_source("https://api.example.com/v1?%61piwatchplaceholder0x={name}");
    let new = phase2_d06_source("https://api.example.com/v1?{name}={name}");
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            old.path().to_str().expect("temporary path should be UTF-8"),
            new.path().to_str().expect("temporary path should be UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn phase2_d06_preserves_network_relative_authority_in_server_changes() {
    let old = phase2_d06_source("//old.example.com/v1");
    let new = phase2_d06_source("//new.example.com/v1");
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            old.path().to_str().expect("temporary path should be UTF-8"),
            new.path().to_str().expect("temporary path should be UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"],
        json!([
            {"severity": "breaking", "method": "GET", "path": "/users", "message": "server //old.example.com/v1 removed"},
            {"severity": "non_breaking", "method": "GET", "path": "/users", "message": "server //new.example.com/v1 added"}
        ])
    );
}

#[test]
fn phase2_d06_preserves_template_ports() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = phase2_d06_source("https://api.example.com:{port}/v1");
    let lock = directory.path().join("port.lock");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            source
                .path()
                .to_str()
                .expect("temporary path should be UTF-8"),
            "--name",
            "port",
            "--output",
            lock.to_str().expect("temporary path should be UTF-8"),
        ])
        .assert()
        .success();

    assert!(std::fs::read_to_string(lock)
        .expect("lock should be readable")
        .contains("https://api.example.com:{port}/v1"));
}

#[test]
fn phase2_d06_does_not_collide_placeholder_tokens_with_literal_url_text() {
    let old = phase2_d06_source("https://api.example.com/apiwatchplaceholder0/{name}");
    let new = phase2_d06_source("https://api.example.com/{name}/{name}");
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            old.path().to_str().expect("temporary path should be UTF-8"),
            new.path().to_str().expect("temporary path should be UTF-8"),
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn phase2_d06_reencodes_query_keys_for_v4_canonical_validation() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = phase2_d06_source("https://api.example.com/v1?a%26b=value");
    let lock = directory.path().join("encoded-key.lock");
    let source_path = source
        .path()
        .to_str()
        .expect("temporary path should be UTF-8");
    let lock_path = lock.to_str().expect("temporary path should be UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            source_path,
            "--name",
            "encoded",
            "--output",
            lock_path,
        ])
        .assert()
        .success();
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "verify",
            source_path,
            "--name",
            "encoded",
            "--lock",
            lock_path,
        ])
        .assert()
        .success();
    assert!(std::fs::read_to_string(lock)
        .expect("lock should be readable")
        .contains("a%26b={redacted}"));
}

#[test]
fn phase2_d06_diff_reports_effective_server_changes_without_leaking_values() {
    let output = Command::cargo_bin("apiwatch")
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

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"],
        json!([
            {
                "severity": "breaking",
                "method": "GET",
                "path": "/removed",
                "message": "server https://api.example.com/v1 removed"
            },
            {
                "severity": "non_breaking",
                "method": "GET",
                "path": "/added",
                "message": "server https://backup.example.com/v1 added"
            }
        ])
    );
    let rendered = String::from_utf8_lossy(&output.stdout);
    assert!(!rendered.contains("tenant=old"));
    assert!(!rendered.contains("tenant=new"));
}

#[test]
fn phase2_d06_rejects_server_credentials_without_echoing_them() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = tempfile::Builder::new()
        .suffix(".yaml")
        .tempfile()
        .expect("temporary source should be created");
    std::fs::write(
        source.path(),
        "openapi: 3.0.3\ninfo: { title: Private, version: '1' }\nservers:\n  - url: https://user:secret@example.com/v1\npaths:\n  /users:\n    get:\n      responses: { '204': { description: ok } }\n",
    )
    .expect("temporary source should be written");
    let lock = directory.path().join("private.lock");

    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            source
                .path()
                .to_str()
                .expect("temporary path should be UTF-8"),
            "--name",
            "private",
            "--output",
            lock.to_str().expect("temporary path should be UTF-8"),
        ])
        .output()
        .expect("Lock command should run");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("credentials"));
    assert!(!stderr.contains("secret"));
    assert!(!std::fs::read(&lock)
        .unwrap_or_default()
        .windows(b"secret".len())
        .any(|bytes| bytes == b"secret"));
}

fn parse_json_output(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be JSON")
}

#[test]
fn phase2_d01_diff_reports_request_body_presence_and_requiredness() {
    let output = Command::cargo_bin("apiwatch")
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

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"],
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
fn phase2_d01_diff_reports_request_body_removal_and_relaxed_requiredness() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/phase2_d01_request_body_new.yaml",
            "testdata/openapi/phase2_d01_request_body_old.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"],
        json!([
            {
                "severity": "breaking",
                "method": "POST",
                "path": "/optional-added",
                "message": "request body removed"
            },
            {
                "severity": "breaking",
                "method": "POST",
                "path": "/required-added",
                "message": "request body removed"
            },
            {
                "severity": "non_breaking",
                "method": "POST",
                "path": "/requiredness",
                "message": "request body changed from required to optional"
            }
        ])
    );
}

#[test]
fn phase2_d02_diff_reports_canonical_media_type_changes() {
    let output = Command::cargo_bin("apiwatch")
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

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"],
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
fn phase2_d02_diff_reports_canonical_media_type_reversals() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/phase2_d02_content_type_new.yaml",
            "testdata/openapi/phase2_d02_content_type_old.yaml",
            "--format",
            "json",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"],
        json!([
            {
                "severity": "breaking",
                "method": "GET",
                "path": "/response",
                "message": "response 200 content type application/problem+json removed"
            },
            {
                "severity": "breaking",
                "method": "POST",
                "path": "/request",
                "message": "request content type application/xml removed"
            },
            {
                "severity": "non_breaking",
                "method": "POST",
                "path": "/request",
                "message": "request content type application/json added"
            }
        ])
    );
}

#[test]
fn phase2_d03_diff_reports_response_requiredness_symmetrically() {
    let output = Command::cargo_bin("apiwatch")
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

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"],
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
fn phase2_d04_diff_reports_additional_properties_direction_and_schema_changes() {
    let output = Command::cargo_bin("apiwatch")
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

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        parse_json_output(&output)["changes"],
        json!([
            {
                "severity": "breaking",
                "method": "GET",
                "path": "/nested-response",
                "message": "response 200 application/json field envelope.additionalProperties changed from forbidden to any"
            },
            {
                "severity": "breaking",
                "method": "GET",
                "path": "/response-broadened",
                "message": "response 200 application/json additionalProperties changed from forbidden to any"
            },
            {
                "severity": "non_breaking",
                "method": "GET",
                "path": "/response-narrowed",
                "message": "response 200 application/json additionalProperties changed from any to forbidden"
            },
            {
                "severity": "breaking",
                "method": "POST",
                "path": "/nested-request",
                "message": "request application/json field envelope.additionalProperties changed from any to forbidden"
            },
            {
                "severity": "non_breaking",
                "method": "POST",
                "path": "/request-broadened",
                "message": "request application/json additionalProperties changed from forbidden to any"
            },
            {
                "severity": "breaking",
                "method": "POST",
                "path": "/request-narrowed",
                "message": "request application/json additionalProperties changed from any to forbidden"
            },
            {
                "severity": "breaking",
                "method": "POST",
                "path": "/typed-map",
                "message": "request application/json additionalProperties type changed from string to integer"
            },
            {
                "severity": "breaking",
                "method": "POST",
                "path": "/typed-map-policy",
                "message": "request application/json field additionalProperties.additionalProperties changed from any to forbidden"
            }
        ])
    );
}

#[test]
fn phase2_d05_diff_reports_schema_format_changes_as_warnings() {
    let output = Command::cargo_bin("apiwatch")
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

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        parse_json_output(&output)["changes"],
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

fn sarif_rule_ids(rendered: &Value) -> Vec<&str> {
    rendered["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("SARIF rules should be an array")
        .iter()
        .map(|rule| rule["id"].as_str().expect("SARIF rule should have an ID"))
        .collect()
}

#[test]
fn diff_sarif_reports_breaking_change_and_exit_one() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/endpoint_removed_old.yaml",
            "testdata/openapi/endpoint_removed_new.yaml",
            "--format",
            "sarif",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));
    let rendered = parse_json_output(&output);
    assert_eq!(
        rendered["$schema"],
        "https://json.schemastore.org/sarif-2.1.0.json"
    );
    assert_eq!(rendered["version"], "2.1.0");
    assert_eq!(rendered["runs"].as_array().map(Vec::len), Some(1));
    assert_eq!(rendered["runs"][0]["tool"]["driver"]["name"], "apiwatch");
    assert_eq!(
        rendered["runs"][0]["tool"]["driver"]["semanticVersion"],
        env!("CARGO_PKG_VERSION")
    );
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
    assert_eq!(
        rendered["runs"][0]["results"],
        json!([{
            "ruleId": "apiwatch/diff-breaking",
            "level": "error",
            "message": { "text": "endpoint removed" },
            "locations": [{
                "physicalLocation": {
                    "artifactLocation": { "uri": "testdata/openapi/endpoint_removed_new.yaml" }
                }
            }],
            "partialFingerprints": {
                "apiwatch/v1": "diff:apiwatch/diff-breaking:GET:/users:endpoint removed"
            }
        }])
    );
}

#[cfg(windows)]
#[test]
fn diff_sarif_normalizes_windows_style_new_artifact_path() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/endpoint_removed_old.yaml",
            "testdata\\openapi\\endpoint_removed_new.yaml",
            "--format",
            "sarif",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(1));
    let rendered = parse_json_output(&output);
    assert_eq!(
        rendered["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
            ["uri"],
        "testdata/openapi/endpoint_removed_new.yaml"
    );
}

#[test]
fn diff_sarif_percent_encodes_new_artifact_path() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/endpoint_removed_old.yaml",
            "testdata/openapi/sarif artifact #%.yaml",
            "--format",
            "sarif",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(1));
    let rendered = parse_json_output(&output);
    assert_eq!(
        rendered["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
            ["uri"],
        "testdata/openapi/sarif%20artifact%20%23%25.yaml"
    );
}

#[test]
fn diff_sarif_reports_warning_only_change_and_exit_zero() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/status_error_added_old.yaml",
            "testdata/openapi/status_error_added_new.yaml",
            "--format",
            "sarif",
        ])
        .output()
        .expect("Diff command should run");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let rendered = parse_json_output(&output);
    let result = &rendered["runs"][0]["results"][0];
    assert_eq!(result["ruleId"], "apiwatch/diff-warning");
    assert_eq!(result["level"], "warning");
    assert_eq!(
        result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "testdata/openapi/status_error_added_new.yaml"
    );
    assert_eq!(
        result["partialFingerprints"]["apiwatch/v1"],
        "diff:apiwatch/diff-warning:GET:/users:response status 429 added"
    );
}

#[test]
fn diff_sarif_reports_no_changes() {
    let output = Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "diff",
            "testdata/openapi/no_breaking_old.yaml",
            "testdata/openapi/no_breaking_old.yaml",
            "--format",
            "sarif",
        ])
        .output()
        .expect("Diff command should run");

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
fn diff_sarif_keeps_invalid_format_rejection() {
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
fn diff_rejects_openapi_31_with_an_accurate_message() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/unsupported_31.yaml",
            "testdata/openapi/unsupported_31.yaml",
        ])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("OpenAPI 3.1 is not yet supported"));
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
fn diff_classifies_oneof_branch_type_replacement() {
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
            "GET /search: response 200 application/json field oneOf[1] added",
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
            "GET /users: response 200 application/json field name removed",
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
            "GET /search: response 200 application/json field anyOf[1].result type changed from string to integer",
        ));
}

#[test]
fn phase2_d09_compares_composition_branches_semantically() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/phase2_d09_composition_old.yaml",
            "testdata/openapi/phase2_d09_composition_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "POST /request-branch-removal: request application/json field oneOf[1] removed",
        ))
        .stdout(predicate::str::contains(
            "GET /response-branch-addition: response 200 application/json field anyOf[1] added",
        ))
        .stdout(predicate::str::contains(
            "GET /allof-required-change: response 200 application/json field name changed from required to optional",
        ))
        .stdout(predicate::str::contains("GET /allof-reordered").not())
        .stdout(predicate::str::contains("GET /allof-empty-neutral").not())
        .stdout(predicate::str::contains("GET /oneof-reordered").not())
        .stdout(predicate::str::contains("GET /anyof-reordered").not())
        .stdout(predicate::str::contains("GET /enum-branch-dedup").not());
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
