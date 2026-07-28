mod canonical;
mod schema;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::contract::{AuthSchemeKind, SchemaKind};
use crate::lockfile::Scope;
use crate::openapi::identity::canonical_media_type;

pub const DEFAULT_MAX_LOCK_BYTES: u64 = 5_242_880;
pub(super) type Extensions = BTreeMap<String, serde_json::Value>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct V4Lock {
    version: u8,
    apis: BTreeMap<String, V4Api>,
}

impl V4Lock {
    pub(super) fn from_parts(
        declared: BTreeMap<String, DeclaredEntry>,
        observed: BTreeMap<String, crate::observed::Shape>,
    ) -> Self {
        let mut apis = declared
            .into_iter()
            .map(|(name, entry)| (name, V4Api::Declared(entry)))
            .collect::<BTreeMap<_, _>>();
        apis.extend(
            observed
                .into_iter()
                .map(|(name, shape)| (name, V4Api::Observed(ObservedEntry { shape }))),
        );
        Self { version: 4, apis }
    }
    pub(super) fn into_parts(
        self,
    ) -> (
        BTreeMap<String, DeclaredEntry>,
        BTreeMap<String, crate::observed::Shape>,
    ) {
        let mut declared = BTreeMap::new();
        let mut observed = BTreeMap::new();
        for (name, api) in self.apis {
            match api {
                V4Api::Declared(entry) => {
                    declared.insert(name, entry);
                }
                V4Api::Observed(entry) => {
                    observed.insert(name, entry.shape);
                }
            }
        }
        (declared, observed)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provenance", rename_all = "snake_case")]
pub(super) enum V4Api {
    Declared(DeclaredEntry),
    Observed(ObservedEntry),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ObservedEntry {
    shape: crate::observed::Shape,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredEntry {
    source: String,
    scope: Scope,
    max_lock_bytes: u64,
    contract_bytes: u64,
    contract_digest: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    extensions: Extensions,
    contract: Contract,
}
impl DeclaredEntry {
    pub(super) fn scope(&self) -> &Scope {
        &self.scope
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Contract {
    operations: BTreeMap<String, WireOperation>,
    schemas: BTreeMap<String, WireSchema>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireOperation {
    display_path: String,
    auth: BTreeMap<String, WireAuth>,
    servers: Vec<String>,
    parameters: BTreeMap<String, WireParameter>,
    request_body: Option<WireRequestBody>,
    responses: BTreeMap<String, BTreeMap<String, String>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireRequestBody {
    required: bool,
    content: BTreeMap<String, String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireAuth {
    kind: AuthSchemeKind,
    scopes: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireParameter {
    #[serde(default)]
    name: String,
    required: bool,
    schema: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireSchema {
    kind: SchemaKind,
    nullable: bool,
    format: Option<String>,
    enum_values: Vec<String>,
    properties: BTreeMap<String, WireProperty>,
    additional_properties: WireAdditionalProperties,
    #[serde(default)]
    branches: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum WireAdditionalProperties {
    Forbidden,
    Any,
    Schema { schema: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireProperty {
    required: bool,
    schema: String,
}

fn contract_yaml(contract: &Contract) -> Result<Vec<u8>> {
    let mut bytes = serde_yaml::to_string(contract)
        .context("failed to serialize v4 declared contract")?
        .into_bytes();
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    Ok(bytes)
}
pub(super) fn build_declared(
    contract: &crate::contract::ApiContract,
    scope: Scope,
    max_lock_bytes: u64,
    extensions: Extensions,
) -> Result<DeclaredEntry> {
    if max_lock_bytes == 0 {
        return Err(anyhow!("max-lock-bytes must be positive"));
    }
    validate_scope(&scope)?;
    canonical::validate_extensions(&extensions)?;
    let contract = schema::intern_contract(contract)?;
    validate_contract_semantics(&contract)?;
    validate_scope_contract(&scope, &contract)?;
    let contract_bytes =
        u64::try_from(contract_yaml(&contract)?.len()).context("contract size overflow")?;
    if contract_bytes > max_lock_bytes {
        return Err(anyhow!(
            "declared contract is {contract_bytes} bytes and exceeds {max_lock_bytes}"
        ));
    }
    let contract_digest = canonical::contract_digest(&scope, &contract, &extensions)?;
    Ok(DeclaredEntry {
        source: "openapi".to_string(),
        scope,
        max_lock_bytes,
        contract_bytes,
        contract_digest,
        extensions,
        contract,
    })
}
pub(super) fn validate_declared(
    name: &str,
    entry: &DeclaredEntry,
) -> Result<crate::contract::ApiContract> {
    validate_name(name)?;
    if entry.source != "openapi" {
        return Err(anyhow!(
            "declared api {name} has unsupported source {}",
            sanitized(&entry.source)
        ));
    }
    if entry.max_lock_bytes == 0 {
        return Err(anyhow!("declared api {name} has a zero max_lock_bytes"));
    }
    validate_scope(&entry.scope)?;
    canonical::validate_extensions(&entry.extensions)?;
    validate_scope_contract(&entry.scope, &entry.contract)?;
    validate_contract_semantics(&entry.contract)?;
    schema::validate_schema_table(&entry.contract)
        .with_context(|| format!("declared api {name} has an invalid schema table"))?;
    let actual_bytes =
        u64::try_from(contract_yaml(&entry.contract)?.len()).context("contract size overflow")?;
    if actual_bytes != entry.contract_bytes {
        return Err(anyhow!("declared api {name} contract byte count mismatch"));
    }
    canonical::validate_digest(&entry.contract_digest)
        .with_context(|| format!("declared api {name} has an invalid contract digest"))?;
    if canonical::contract_digest(&entry.scope, &entry.contract, &entry.extensions)?
        != entry.contract_digest
    {
        return Err(anyhow!("declared api {name} contract digest mismatch"));
    }
    schema::expand_contract(&entry.contract)
        .with_context(|| format!("failed to reconstruct declared api {name}"))
}
pub(super) fn render(lock: &V4Lock) -> Result<String> {
    validate_lock(lock)?;
    serde_yaml::to_string(lock).context("failed to serialize api.lock v4")
}
pub(super) fn load(contents: &str) -> Result<V4Lock> {
    let raw: serde_yaml::Value =
        serde_yaml::from_str(contents).context("failed to parse api.lock v4 YAML")?;
    validate_raw_observed_shapes(&raw)?;
    validate_raw_additional_properties(&raw)?;
    let lock = serde_yaml::from_value(raw).context("failed to parse api.lock v4 YAML")?;
    validate_lock(&lock)?;
    Ok(lock)
}
fn validate_lock(lock: &V4Lock) -> Result<()> {
    if lock.version != 4 {
        return Err(anyhow!("unsupported api.lock version {}", lock.version));
    }
    for (name, api) in &lock.apis {
        validate_name(name)?;
        if let V4Api::Declared(entry) = api {
            validate_declared(name, entry)?;
        }
    }
    Ok(())
}
fn validate_raw_observed_shapes(raw: &serde_yaml::Value) -> Result<()> {
    let Some(apis) = mapping_value(raw, "apis").and_then(serde_yaml::Value::as_mapping) else {
        return Ok(());
    };
    for api in apis.values().filter_map(serde_yaml::Value::as_mapping) {
        if string_value(api, "provenance") == Some("observed") {
            if let Some(shape) = mapping_value_from(api, "shape") {
                validate_raw_shape(shape)?;
            }
        }
    }
    Ok(())
}
fn validate_raw_additional_properties(raw: &serde_yaml::Value) -> Result<()> {
    let Some(apis) = mapping_value(raw, "apis").and_then(serde_yaml::Value::as_mapping) else {
        return Ok(());
    };
    for api in apis.values().filter_map(serde_yaml::Value::as_mapping) {
        if string_value(api, "provenance") != Some("declared") {
            continue;
        }
        let Some(contract) =
            mapping_value_from(api, "contract").and_then(serde_yaml::Value::as_mapping)
        else {
            continue;
        };
        let Some(schemas) =
            mapping_value_from(contract, "schemas").and_then(serde_yaml::Value::as_mapping)
        else {
            continue;
        };
        for schema in schemas.values().filter_map(serde_yaml::Value::as_mapping) {
            if let Some(policy) = mapping_value_from(schema, "additional_properties") {
                validate_raw_additional_properties_policy(policy)?;
            }
        }
    }
    Ok(())
}
fn validate_raw_additional_properties_policy(value: &serde_yaml::Value) -> Result<()> {
    let Some(policy) = value.as_mapping() else {
        return Ok(());
    };
    let Some(kind) = string_value(policy, "kind") else {
        return Ok(());
    };
    let allowed: &[&str] = match kind {
        "forbidden" | "any" => &["kind"],
        "schema" => &["kind", "schema"],
        _ => return Ok(()),
    };
    if policy
        .keys()
        .any(|key| key.as_str().is_some_and(|key| !allowed.contains(&key)))
    {
        return Err(anyhow!("unknown field in additionalProperties policy"));
    }
    Ok(())
}
fn validate_raw_shape(value: &serde_yaml::Value) -> Result<()> {
    let Some(shape) = value.as_mapping() else {
        return Ok(());
    };
    let Some(kind) = string_value(shape, "kind") else {
        return Ok(());
    };
    let allowed: &[&str] = match kind {
        "object" => &["kind", "observations", "properties"],
        "map" => &["kind", "values"],
        "array" => &["kind", "items"],
        "union" => &["kind", "variants"],
        _ => &["kind"],
    };
    if shape
        .keys()
        .any(|key| key.as_str().is_some_and(|key| !allowed.contains(&key)))
    {
        return Err(anyhow!("unknown field in observed shape"));
    }
    match kind {
        "object" => {
            if let Some(properties) =
                mapping_value_from(shape, "properties").and_then(serde_yaml::Value::as_mapping)
            {
                for property in properties
                    .values()
                    .filter_map(serde_yaml::Value::as_mapping)
                {
                    if property.keys().any(|key| {
                        key.as_str()
                            .is_some_and(|key| !["observations", "shape"].contains(&key))
                    }) {
                        return Err(anyhow!("unknown field in observed shape"));
                    }
                    if let Some(nested) = mapping_value_from(property, "shape") {
                        validate_raw_shape(nested)?;
                    }
                }
            }
        }
        "map" => {
            if let Some(values) = mapping_value_from(shape, "values") {
                validate_raw_shape(values)?;
            }
        }
        "array" => {
            if let Some(items) = mapping_value_from(shape, "items") {
                validate_raw_shape(items)?;
            }
        }
        "union" => {
            if let Some(variants) =
                mapping_value_from(shape, "variants").and_then(serde_yaml::Value::as_sequence)
            {
                for variant in variants {
                    validate_raw_shape(variant)?;
                }
            }
        }
        _ => {}
    };
    Ok(())
}
fn mapping_value<'a>(value: &'a serde_yaml::Value, key: &str) -> Option<&'a serde_yaml::Value> {
    value
        .as_mapping()
        .and_then(|mapping| mapping_value_from(mapping, key))
}
fn mapping_value_from<'a>(
    mapping: &'a serde_yaml::Mapping,
    key: &str,
) -> Option<&'a serde_yaml::Value> {
    mapping.get(serde_yaml::Value::String(key.to_string()))
}
fn string_value<'a>(mapping: &'a serde_yaml::Mapping, key: &str) -> Option<&'a str> {
    mapping_value_from(mapping, key).and_then(serde_yaml::Value::as_str)
}
fn validate_scope(scope: &Scope) -> Result<()> {
    let Scope::Operations(_) = scope else {
        return Ok(());
    };
    if scope.selectors().is_empty() {
        return Err(anyhow!("operation scope cannot be empty"));
    }
    let mut previous = None;
    for selector in scope.selectors() {
        let key = crate::lock_size::parse_operation_selector(selector)?;
        let canonical = format!("{} {}", key.method.as_str(), key.path);
        if selector != &canonical {
            return Err(anyhow!("operation scope selector is not canonical"));
        }
        if previous.as_ref().is_some_and(|value| value >= &key) {
            return Err(anyhow!(
                "operation scope must be sorted and contain no duplicates"
            ));
        }
        previous = Some(key);
    }
    Ok(())
}
fn validate_scope_contract(scope: &Scope, contract: &Contract) -> Result<()> {
    let Scope::Operations(_) = scope else {
        return Ok(());
    };
    let scoped = scope
        .selectors()
        .iter()
        .map(|selector| crate::lock_size::parse_operation_selector(selector))
        .collect::<Result<BTreeSet<_>>>()?;
    let stored = contract
        .operations
        .keys()
        .map(|key| schema::parse_operation_key(key))
        .collect::<Result<BTreeSet<_>>>()?;
    if scoped != stored {
        return Err(anyhow!(
            "operation scope does not match the stored contract operations"
        ));
    }
    Ok(())
}
fn validate_contract_semantics(contract: &Contract) -> Result<()> {
    for (operation_key, operation) in &contract.operations {
        let parsed = schema::parse_operation_key(operation_key)?;
        if operation_key != &format!("{} {}", parsed.method.as_str(), parsed.path) {
            return Err(anyhow!("operation key is not canonical"));
        }
        let (display_identity, display_placeholders) =
            crate::openapi::identity::canonical_path_template(&operation.display_path)?;
        if display_identity != parsed.path {
            return Err(anyhow!(
                "operation display path does not match its canonical identity"
            ));
        }
        for (name, auth) in &operation.auth {
            validate_wire_string(name, "auth name", false)?;
            validate_normalized_strings(&auth.scopes, "auth scopes")?;
        }
        validate_normalized_strings(&operation.servers, "servers")?;
        for server in &operation.servers {
            if crate::openapi::identity::canonical_server_template(server)?.0 != *server {
                return Err(anyhow!("server template is not canonical"));
            }
        }
        let mut expected_path_parameters = display_placeholders
            .iter()
            .enumerate()
            .map(|(slot, name)| (format!("{{{slot}}}"), name))
            .collect::<BTreeMap<_, _>>();
        for (parameter_key, parameter) in &operation.parameters {
            let parsed = schema::parse_parameter_key(parameter_key)?;
            if parameter_key != &format!("{}:{}", parsed.location.as_str(), parsed.name) {
                return Err(anyhow!("parameter key is not canonical"));
            }
            if parsed.location == crate::contract::ParameterLocation::Path
                && expected_path_parameters.remove(&parsed.name) != Some(&parameter.name)
            {
                return Err(anyhow!("path parameter bindings do not match display path"));
            }
        }
        if !expected_path_parameters.is_empty() {
            return Err(anyhow!("path parameter bindings do not match display path"));
        }
        if let Some(body) = &operation.request_body {
            validate_content_types(&body.content)?;
        }
        for (status, content) in &operation.responses {
            validate_wire_string(status, "response status", false)?;
            validate_content_types(content)?;
        }
    }
    for schema in contract.schemas.values() {
        if let Some(format) = &schema.format {
            validate_wire_string(format, "schema format", true)?;
        }
        validate_normalized_strings(&schema.enum_values, "schema enum values")?;
        validate_normalized_strings(&schema.branches, "schema branches")?;
        if !schema.branches.is_empty()
            && !matches!(schema.kind, SchemaKind::OneOf | SchemaKind::AnyOf)
        {
            return Err(anyhow!(
                "schema branches are only valid for oneOf or anyOf schemas"
            ));
        }
        for property in schema.properties.keys() {
            validate_wire_string(property, "schema property name", true)?;
        }
    }
    Ok(())
}
fn validate_content_types(content: &BTreeMap<String, String>) -> Result<()> {
    for content_type in content.keys() {
        validate_wire_string(content_type, "media type", false)?;
        if canonical_media_type(content_type)? != *content_type {
            return Err(anyhow!("media type is not canonical"));
        }
    }
    Ok(())
}
fn validate_normalized_strings(values: &[String], label: &str) -> Result<()> {
    for value in values {
        validate_wire_string(value, label, true)?;
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(anyhow!("{label} must be sorted and contain no duplicates"));
    }
    Ok(())
}
fn validate_wire_string(value: &str, label: &str, allow_empty: bool) -> Result<()> {
    if !allow_empty && value.is_empty() {
        return Err(anyhow!("{label} cannot be empty"));
    }
    if value.chars().any(char::is_control) {
        return Err(anyhow!("{label} contains a control character"));
    }
    Ok(())
}
fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(anyhow!("api name cannot be empty"));
    }
    if name != name.trim() {
        return Err(anyhow!("api name has surrounding whitespace"));
    }
    if name.chars().any(char::is_control) {
        return Err(anyhow!("api name contains a control character"));
    }
    Ok(())
}
fn sanitized(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}
