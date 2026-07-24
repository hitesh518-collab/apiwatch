use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::contract::{
    ApiContract, AuthRequirement, HttpMethod, Operation, OperationKey, ParameterKey, Schema,
    SchemaKind,
};

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct DeduplicatedContract {
    operations: BTreeMap<OperationKey, DeduplicatedOperation>,
    schemas: BTreeMap<String, WireSchema>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct DeduplicatedOperation {
    auth: BTreeMap<String, AuthRequirement>,
    parameters: BTreeMap<ParameterKey, WireParameter>,
    request_body: Option<BTreeMap<String, String>>,
    responses: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct WireParameter {
    name: String,
    required: bool,
    schema: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct WireSchema {
    kind: SchemaKind,
    nullable: bool,
    format: Option<String>,
    enum_values: Vec<String>,
    properties: BTreeMap<String, WireProperty>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct WireProperty {
    required: bool,
    schema: String,
}

pub fn sha256_id(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn intern_schema<F>(
    schema: &Schema,
    schemas: &mut BTreeMap<String, WireSchema>,
    canonical: &mut BTreeMap<String, Vec<u8>>,
    digest: &F,
) -> Result<String>
where
    F: Fn(&[u8]) -> String,
{
    let mut properties = BTreeMap::new();
    for (name, property) in &schema.properties {
        let id = intern_schema(&property.schema, schemas, canonical, digest)?;
        properties.insert(
            name.clone(),
            WireProperty {
                required: property.required,
                schema: id,
            },
        );
    }
    let wire = WireSchema {
        kind: schema.kind.clone(),
        nullable: schema.nullable,
        format: schema.format.clone(),
        enum_values: schema.enum_values.clone(),
        properties,
    };
    let bytes = serde_json::to_vec(&wire).context("failed to canonicalize schema")?;
    let id = digest(&bytes);
    if let Some(existing) = canonical.get(&id) {
        if existing != &bytes {
            return Err(anyhow!("schema digest collision"));
        }
    } else {
        canonical.insert(id.clone(), bytes);
        schemas.insert(id.clone(), wire);
    }
    Ok(id)
}

fn deduplicate_operation<F>(
    operation: &Operation,
    schemas: &mut BTreeMap<String, WireSchema>,
    canonical: &mut BTreeMap<String, Vec<u8>>,
    digest: &F,
) -> Result<DeduplicatedOperation>
where
    F: Fn(&[u8]) -> String,
{
    let parameters = operation
        .parameters
        .iter()
        .map(|(key, parameter)| {
            let schema = intern_schema(&parameter.schema, schemas, canonical, digest)?;
            Ok((
                key.clone(),
                WireParameter {
                    name: parameter.name.clone(),
                    required: parameter.required,
                    schema,
                },
            ))
        })
        .collect::<Result<_>>()?;
    let request_body = operation
        .request_body
        .as_ref()
        .map(|body| {
            body.content
                .iter()
                .map(|(content_type, schema)| {
                    Ok((
                        content_type.clone(),
                        intern_schema(schema, schemas, canonical, digest)?,
                    ))
                })
                .collect::<Result<_>>()
        })
        .transpose()?;
    let responses = operation
        .responses
        .iter()
        .map(|(status, response)| {
            let content = response
                .content
                .iter()
                .map(|(content_type, schema)| {
                    Ok((
                        content_type.clone(),
                        intern_schema(schema, schemas, canonical, digest)?,
                    ))
                })
                .collect::<Result<_>>()?;
            Ok((status.clone(), content))
        })
        .collect::<Result<_>>()?;
    Ok(DeduplicatedOperation {
        auth: operation.auth.clone(),
        parameters,
        request_body,
        responses,
    })
}

fn deduplicate_with<F>(contract: &ApiContract, digest: &F) -> Result<DeduplicatedContract>
where
    F: Fn(&[u8]) -> String,
{
    let mut schemas = BTreeMap::new();
    let mut canonical = BTreeMap::new();
    let operations = contract
        .operations
        .iter()
        .map(|(key, operation)| {
            Ok((
                key.clone(),
                deduplicate_operation(operation, &mut schemas, &mut canonical, digest)?,
            ))
        })
        .collect::<Result<_>>()?;
    Ok(DeduplicatedContract {
        operations,
        schemas,
    })
}

pub fn encode_deduplicated_yaml(contract: &ApiContract) -> Result<Vec<u8>> {
    let wire = deduplicate_with(contract, &sha256_id)?;
    let mut rendered = serde_yaml::to_string(&wire)
        .context("failed to encode deduplicated YAML")?
        .into_bytes();
    if !rendered.ends_with(b"\n") {
        rendered.push(b'\n');
    }
    Ok(rendered)
}

#[cfg(test)]
fn intern_schemas_for_test<F>(input: &[Schema], digest: F) -> Result<()>
where
    F: Fn(&[u8]) -> String,
{
    let mut schemas = BTreeMap::new();
    let mut canonical = BTreeMap::new();
    for schema in input {
        intern_schema(schema, &mut schemas, &mut canonical, &digest)?;
    }
    Ok(())
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
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::{
        encode_canonical_json, encode_deduplicated_yaml, encode_expanded_yaml,
        intern_schemas_for_test, parse_operation_selector, scope_contract, sha256_id,
        PRIVACY_SENTINELS,
    };
    use crate::contract::{HttpMethod, Schema, SchemaKind};
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

    #[test]
    fn sha256_ids_are_stable_and_prefixed() {
        assert_eq!(
            sha256_id(b"schema"),
            "sha256:df0ad6e43880f09c90ebf95f19110178aba6890df0010ebda7485029e2b543b4"
        );
    }

    #[test]
    fn deduplicated_yaml_interns_schemas_and_is_private() {
        let contract = load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml")).unwrap();
        let first = encode_deduplicated_yaml(&contract).unwrap();
        let second = encode_deduplicated_yaml(&contract).unwrap();
        assert_eq!(first, second);
        let text = String::from_utf8(first).unwrap();
        assert!(text.contains("schemas:"));
        assert!(text.contains("sha256:"));
        for sentinel in PRIVACY_SENTINELS {
            assert!(!text.contains(sentinel));
        }
    }

    #[test]
    fn deduplication_rejects_a_forced_digest_collision() {
        let first = Schema {
            kind: SchemaKind::String,
            nullable: false,
            format: None,
            enum_values: Vec::new(),
            properties: BTreeMap::new(),
        };
        let second = Schema {
            kind: SchemaKind::Boolean,
            nullable: false,
            format: None,
            enum_values: Vec::new(),
            properties: BTreeMap::new(),
        };
        let error =
            intern_schemas_for_test(&[first, second], |_| "sha256:forced".into()).unwrap_err();
        assert!(error.to_string().contains("schema digest collision"));
    }
}
