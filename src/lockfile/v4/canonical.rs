use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{Contract, Extensions, WireSchema};
use crate::lockfile::Scope;

const SCHEMA_DOMAIN: &str = "apiwatch.schema.v4";
const CONTRACT_DOMAIN: &str = "apiwatch.declared-contract.v4";

#[derive(Serialize)]
struct SchemaDigestInput<'a> {
    domain: &'static str,
    schema: &'a WireSchema,
}
#[derive(Serialize)]
struct ContractDigestInput<'a> {
    domain: &'static str,
    scope: &'a Scope,
    contract: &'a Contract,
    extensions: &'a Extensions,
}

pub(super) fn schema_id(schema: &WireSchema) -> Result<String> {
    let bytes = serde_json::to_vec(&SchemaDigestInput {
        domain: SCHEMA_DOMAIN,
        schema,
    })
    .context("failed to canonicalize v4 schema")?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

pub(super) fn contract_digest(
    scope: &Scope,
    contract: &Contract,
    extensions: &Extensions,
) -> Result<String> {
    validate_extensions(extensions)?;
    let extensions = extensions
        .iter()
        .map(|(key, value)| (key.clone(), canonical_value(value)))
        .collect();
    let bytes = serde_json::to_vec(&ContractDigestInput {
        domain: CONTRACT_DOMAIN,
        scope,
        contract,
        extensions: &extensions,
    })
    .context("failed to canonicalize v4 contract")?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

pub(super) fn validate_extensions(extensions: &Extensions) -> Result<()> {
    if let Some(key) = extensions.keys().find(|key| !key.starts_with("x-")) {
        return Err(anyhow!(
            "extension key must start with x-: {}",
            sanitized(key)
        ));
    }
    for (key, value) in extensions {
        validate_extension_string(key)?;
        validate_extension_value(value)?;
    }
    Ok(())
}
fn validate_extension_value(value: &Value) -> Result<()> {
    match value {
        Value::String(value) => validate_extension_string(value),
        Value::Array(values) => {
            for value in values {
                validate_extension_value(value)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for (key, value) in values {
                validate_extension_string(key)?;
                validate_extension_value(value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
fn validate_extension_string(value: &str) -> Result<()> {
    if value.chars().any(char::is_control) {
        return Err(anyhow!("extension contains a control character"));
    }
    Ok(())
}
pub(super) fn validate_digest(value: &str) -> Result<()> {
    let hex = value
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow!("digest must start with sha256:"))?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(anyhow!(
            "digest must contain 64 lowercase hexadecimal characters"
        ));
    }
    Ok(())
}
fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(canonical_value).collect()),
        Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), canonical_value(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        value => value.clone(),
    }
}
fn sanitized(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}
