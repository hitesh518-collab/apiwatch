use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use assert_cmd::Command;
use predicates::prelude::*;

fn corpus_file(filename: &str) -> PathBuf {
    let root =
        std::env::var_os("APIWATCH_COMPAT_DIR").unwrap_or_else(|| OsString::from(".compat-cache"));
    let path = PathBuf::from(root).join(filename);
    assert!(
        path.is_file(),
        "missing compatibility fixture {}; run python scripts/fetch_compat_specs.py",
        path.display()
    );
    path
}

fn assert_clean_self_diff(filename: &str) {
    let path = corpus_file(filename);
    let contract =
        apiwatch::openapi::load_contract(&path).expect("compatibility contract should normalize");
    let payload_bytes = apiwatch::lockfile::measure_v4_contract_payload(&contract)
        .expect("production v4 contract payload should serialize");
    assert!(
        payload_bytes <= apiwatch::lockfile::DEFAULT_MAX_LOCK_BYTES,
        "{filename} v4 contract payload is {payload_bytes} bytes and exceeds {}",
        apiwatch::lockfile::DEFAULT_MAX_LOCK_BYTES
    );
    let path = path.to_str().expect("compatibility path should be UTF-8");
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["diff", path, path])
        .assert()
        .success()
        .stdout("No changes detected.\n")
        .stderr(predicate::str::is_empty());
}

fn assert_known_failure(filename: &str, expected_error: &str) {
    let path = corpus_file(filename);
    let path = path.to_str().expect("compatibility path should be UTF-8");
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["diff", path, path])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(expected_error));
}

fn assert_known_failure_with_timeout(filename: &str, expected_error: &str, timeout: Duration) {
    let path = corpus_file(filename);
    let path = path.to_str().expect("compatibility path should be UTF-8");
    let path = path.to_string();
    let expected_error = expected_error.to_string();

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let bin = std::env::var("CARGO_BIN_EXE_apiwatch")
            .unwrap_or_else(|_| format!("target/release/apiwatch{}", std::env::consts::EXE_SUFFIX));
        let output = std::process::Command::new(bin)
            .args(["diff", &path, &path])
            .output();
        tx.send(output).ok();
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                output.status.code(),
                Some(2),
                "expected exit code 2, got {:?}\nstderr: {}\nstdout: {}",
                output.status.code(),
                stderr,
                String::from_utf8_lossy(&output.stdout)
            );
            assert!(
                stderr.contains(&expected_error),
                "expected '{}' in stderr, got '{}'",
                expected_error,
                stderr
            );
        }
        Ok(Err(e)) => panic!("failed to execute apiwatch: {e}"),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            panic!(
                "test timed out after {}s (likely D-28: Stripe schema expansion hangs)",
                timeout.as_secs()
            );
        }
        Err(_) => panic!("thread panicked"),
    }
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn github_rest_is_compatible() {
    assert_clean_self_diff("github.json");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn asana_is_compatible() {
    assert_clean_self_diff("asana.yaml");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn box_is_compatible() {
    assert_clean_self_diff("box.json");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn stripe_reproduces_known_recursive_schema_failure() {
    assert_known_failure_with_timeout(
        "stripe.json",
        "circular schema reference detected: #/components/schemas/file",
        Duration::from_secs(30),
    );
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn digitalocean_reproduces_known_metadata_failure() {
    assert_known_failure("digitalocean.yaml", "missing field `responses`");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn paystack_reproduces_known_unsupported_path_ref_failure() {
    assert_known_failure("paystack.yaml", "unsupported schema reference:");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn deutsche_bahn_reproduces_known_swagger_v2_parsing_failure() {
    assert_known_failure("deutsche-bahn.yaml", "failed to parse cleaned OpenAPI YAML");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn mercadopago_is_compatible() {
    assert_clean_self_diff("mercadopago.yaml");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn line_is_compatible() {
    assert_clean_self_diff("line.yml");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn humanitas_fhir_is_compatible() {
    assert_clean_self_diff("humanitas-fhir.json");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn petstore_is_compatible() {
    assert_clean_self_diff("petstore.yaml");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn plaid_is_compatible() {
    assert_clean_self_diff("plaid.yml");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn shopify_is_compatible() {
    assert_clean_self_diff("shopify.json");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn twilio_is_compatible() {
    assert_clean_self_diff("twilio.yaml");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn adyen_is_compatible() {
    assert_clean_self_diff("adyen-checkout.json");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn kubernetes_is_compatible() {
    assert_clean_self_diff("kubernetes.json");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn intercom_is_compatible() {
    assert_clean_self_diff("intercom.yaml");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn slack_reproduces_known_parameter_schema_failure() {
    assert_known_failure("slack.json", "no variant of enum ParameterSchemaOrContent");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn figma_reproduces_known_duplicate_auth_failure() {
    assert_known_failure("figma.yaml", "duplicate authentication identity");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn openai_reproduces_known_yaml_parse_failure() {
    assert_known_failure("openai.yaml", "failed to parse OpenAPI YAML");
}
