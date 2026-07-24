use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use crate::contract::{ApiContract, HttpMethod, OperationKey};

pub const PRIVACY_SENTINELS: &[&str] = &[
    "APIWATCH_DESCRIPTION_SENTINEL",
    "APIWATCH_EXTENSION_SENTINEL",
    "APIWATCH_OPERATION_DESCRIPTION_SENTINEL",
    "APIWATCH_RESPONSE_DESCRIPTION_SENTINEL",
    "APIWATCH_DEFAULT_SENTINEL",
    "APIWATCH_EXAMPLE_SENTINEL",
    "APIWATCH_CREDENTIAL_SENTINEL",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateKind {
    ExpandedYaml,
    CanonicalJson,
    DeduplicatedYaml,
}

pub fn encode_expanded_yaml(contract: &ApiContract) -> Result<Vec<u8>> {
    let mut rendered = serde_yaml::to_string(contract)
        .context("failed to encode expanded YAML")?
        .into_bytes();
    if !rendered.ends_with(b"\n") {
        rendered.push(b'\n');
    }
    Ok(rendered)
}

pub fn encode_canonical_json(contract: &ApiContract) -> Result<Vec<u8>> {
    let mut rendered = serde_json::to_vec(contract).context("failed to encode canonical JSON")?;
    rendered.push(b'\n');
    Ok(rendered)
}

pub fn parse_operation_selector(value: &str) -> Result<OperationKey> {
    let Some((method, path)) = value.split_once(' ') else {
        return Err(anyhow!("operation selector must be METHOD /path"));
    };
    if method.is_empty() || path.is_empty() || path.contains(' ') {
        return Err(anyhow!("operation selector must contain one ASCII space"));
    }
    let method = match method.to_ascii_uppercase().as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "OPTIONS" => HttpMethod::Options,
        "HEAD" => HttpMethod::Head,
        "TRACE" => HttpMethod::Trace,
        _ => return Err(anyhow!("unsupported operation selector method")),
    };
    if !path.starts_with('/') || path.chars().any(char::is_control) {
        return Err(anyhow!(
            "operation selector path must be a safe absolute path"
        ));
    }
    Ok(OperationKey {
        method,
        path: path.to_owned(),
    })
}

pub fn scope_contract(contract: &ApiContract, selectors: &[String]) -> Result<ApiContract> {
    if selectors.is_empty() {
        return Ok(contract.clone());
    }
    let mut selected = BTreeSet::new();
    for selector in selectors {
        let key = parse_operation_selector(selector)?;
        if !selected.insert(key) {
            return Err(anyhow!("duplicate operation selector"));
        }
    }
    let mut operations = BTreeMap::new();
    for key in selected {
        let operation = contract
            .operations
            .get(&key)
            .ok_or_else(|| anyhow!("operation selector was not found"))?;
        operations.insert(key, operation.clone());
    }
    Ok(ApiContract { operations })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        encode_canonical_json, encode_expanded_yaml, parse_operation_selector, scope_contract,
        PRIVACY_SENTINELS,
    };
    use crate::contract::HttpMethod;
    use crate::openapi::load_contract;

    #[test]
    fn selector_normalizes_method_and_preserves_exact_path() {
        let key = parse_operation_selector("get /users").unwrap();
        assert_eq!(key.method, HttpMethod::Get);
        assert_eq!(key.path, "/users");
    }

    #[test]
    fn selector_rejects_ambiguous_whitespace_and_invalid_paths() {
        for value in [
            "GET  /users",
            " GET /users",
            "GET /users ",
            "GET users",
            "BOGUS /users",
            "GET /users\u{0001}",
        ] {
            assert!(parse_operation_selector(value).is_err(), "{value:?}");
        }
    }

    #[test]
    fn scope_rejects_duplicates_and_missing_operations() {
        let contract = load_contract(Path::new("testdata/openapi/verify_matching.yaml")).unwrap();
        assert!(
            scope_contract(&contract, &["get /users".into(), "GET /users".into()])
                .unwrap_err()
                .to_string()
                .contains("duplicate operation selector")
        );
        assert!(scope_contract(&contract, &["DELETE /missing".into()])
            .unwrap_err()
            .to_string()
            .contains("operation selector was not found"));
    }

    #[test]
    fn empty_selectors_clone_the_full_contract() {
        let contract = load_contract(Path::new("testdata/openapi/verify_matching.yaml")).unwrap();
        assert_eq!(scope_contract(&contract, &[]).unwrap(), contract);
    }

    #[test]
    fn expanded_encoders_are_deterministic_and_value_free() {
        let contract = load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml")).unwrap();
        for rendered in [
            encode_expanded_yaml(&contract).unwrap(),
            encode_canonical_json(&contract).unwrap(),
        ] {
            assert_eq!(rendered.last(), Some(&b'\n'));
            let second = if rendered.starts_with(b"{") {
                encode_canonical_json(&contract).unwrap()
            } else {
                encode_expanded_yaml(&contract).unwrap()
            };
            assert_eq!(rendered, second);
            let text = String::from_utf8(rendered).unwrap();
            for sentinel in PRIVACY_SENTINELS {
                assert!(!text.contains(sentinel));
            }
            assert!(text.contains("/accounts"));
            assert!(text.contains("token"));
        }
    }
}
