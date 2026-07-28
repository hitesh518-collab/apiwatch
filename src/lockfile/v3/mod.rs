mod canonical;
mod schema;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

pub use super::Scope;
use crate::contract::{AuthSchemeKind, SchemaKind};

pub const DEFAULT_MAX_LOCK_BYTES: u64 = 5_242_880;

pub(super) type Extensions = BTreeMap<String, serde_json::Value>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct V3Lock {
    version: u8,
    apis: BTreeMap<String, V3Api>,
}

impl V3Lock {
    #[cfg(test)]
    pub(super) fn single_declared(name: &str, entry: DeclaredEntry) -> Result<Self> {
        validate_name(name)?;
        Ok(Self {
            version: 3,
            apis: BTreeMap::from([(name.to_string(), V3Api::Declared(entry))]),
        })
    }

    #[cfg(test)]
    pub(super) fn declared(&self, name: &str) -> Option<&DeclaredEntry> {
        match self.apis.get(name) {
            Some(V3Api::Declared(entry)) => Some(entry),
            _ => None,
        }
    }

    pub(super) fn from_parts(
        declared: BTreeMap<String, DeclaredEntry>,
        observed: BTreeMap<String, crate::observed::Shape>,
    ) -> Self {
        let mut apis = declared
            .into_iter()
            .map(|(name, entry)| (name, V3Api::Declared(entry)))
            .collect::<BTreeMap<_, _>>();
        apis.extend(
            observed
                .into_iter()
                .map(|(name, shape)| (name, V3Api::Observed(ObservedEntry { shape }))),
        );
        Self { version: 3, apis }
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
                V3Api::Declared(entry) => {
                    declared.insert(name, entry);
                }
                V3Api::Observed(entry) => {
                    observed.insert(name, entry.shape);
                }
            }
        }
        (declared, observed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provenance", rename_all = "snake_case")]
pub(super) enum V3Api {
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
struct WireOperation {
    auth: BTreeMap<String, WireAuth>,
    parameters: BTreeMap<String, WireParameter>,
    request_body: Option<BTreeMap<String, String>>,
    responses: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAuth {
    kind: AuthSchemeKind,
    scopes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireParameter {
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
}

impl WireSchema {
    #[cfg(test)]
    pub(super) fn unknown() -> Self {
        Self {
            kind: SchemaKind::Unknown,
            nullable: false,
            format: None,
            enum_values: Vec::new(),
            properties: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireProperty {
    required: bool,
    schema: String,
}

pub(super) fn contract_yaml(contract: &Contract) -> Result<Vec<u8>> {
    let mut bytes = serde_yaml::to_string(contract)
        .context("failed to serialize v3 declared contract")?
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
    let actual_digest =
        canonical::contract_digest(&entry.scope, &entry.contract, &entry.extensions)?;
    if actual_digest != entry.contract_digest {
        return Err(anyhow!("declared api {name} contract digest mismatch"));
    }
    schema::expand_contract(&entry.contract)
        .with_context(|| format!("failed to reconstruct declared api {name}"))
}

pub(super) fn render(lock: &V3Lock) -> Result<String> {
    validate_lock(lock)?;
    serde_yaml::to_string(lock).context("failed to serialize api.lock v3")
}

pub(super) fn load(contents: &str) -> Result<V3Lock> {
    let raw: serde_yaml::Value =
        serde_yaml::from_str(contents).context("failed to parse api.lock v3 YAML")?;
    validate_raw_observed_shapes(&raw)?;
    let mut lock: V3Lock =
        serde_yaml::from_value(raw).context("failed to parse api.lock v3 YAML")?;
    for api in lock.apis.values_mut() {
        if let V3Api::Declared(entry) = api {
            validate_legacy_scope_digest(entry)?;
            canonicalize_scope_on_load(&mut entry.scope)?;
            entry.contract_digest =
                canonical::contract_digest(&entry.scope, &entry.contract, &entry.extensions)?;
        }
    }
    validate_lock(&lock)?;
    Ok(lock)
}

fn validate_legacy_scope_digest(entry: &DeclaredEntry) -> Result<()> {
    let Scope::Operations(scope) = &entry.scope else {
        return Ok(());
    };
    let mut previous = None;
    for selector in &scope.operations {
        let identity = crate::lock_size::parse_operation_selector(selector)?;
        if previous.as_ref().is_some_and(|value| value >= &identity) {
            return Err(anyhow!(
                "operation scope must be sorted and contain no duplicates"
            ));
        }
        previous = Some(identity);
    }
    canonical::validate_digest(&entry.contract_digest)?;
    if canonical::contract_digest(&entry.scope, &entry.contract, &entry.extensions)?
        != entry.contract_digest
    {
        return Err(anyhow!("declared api contract digest mismatch"));
    }
    Ok(())
}

fn canonicalize_scope_on_load(scope: &mut Scope) -> Result<()> {
    let Scope::Operations(scope) = scope else {
        return Ok(());
    };
    scope.operations = scope
        .operations
        .iter()
        .map(|selector| crate::lock_size::parse_operation_selector(selector))
        .collect::<Result<BTreeSet<_>>>()?
        .into_iter()
        .map(|identity| format!("{} {}", identity.method.as_str(), identity.path))
        .collect();
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
    reject_unknown_mapping_keys(shape, allowed)?;
    match kind {
        "object" => {
            if let Some(properties) =
                mapping_value_from(shape, "properties").and_then(serde_yaml::Value::as_mapping)
            {
                for property in properties
                    .values()
                    .filter_map(serde_yaml::Value::as_mapping)
                {
                    reject_unknown_mapping_keys(property, &["observations", "shape"])?;
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
    }
    Ok(())
}

fn reject_unknown_mapping_keys(mapping: &serde_yaml::Mapping, allowed: &[&str]) -> Result<()> {
    if mapping
        .keys()
        .any(|key| key.as_str().is_some_and(|key| !allowed.contains(&key)))
    {
        return Err(anyhow!("unknown field in observed shape"));
    }
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

fn validate_lock(lock: &V3Lock) -> Result<()> {
    if lock.version != 3 {
        return Err(anyhow!("unsupported api.lock version {}", lock.version));
    }
    for (name, api) in &lock.apis {
        validate_name(name)?;
        if let V3Api::Declared(entry) = api {
            validate_declared(name, entry)?;
        }
    }
    Ok(())
}

fn validate_scope(scope: &Scope) -> Result<()> {
    let Scope::Operations(scope) = scope else {
        return Ok(());
    };
    if scope.operations.is_empty() {
        return Err(anyhow!("operation scope cannot be empty"));
    }
    let mut previous = None;
    for selector in &scope.operations {
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
    let Scope::Operations(scope) = scope else {
        return Ok(());
    };
    let scoped = scope
        .operations
        .iter()
        .map(|selector| crate::lock_size::parse_operation_selector(selector))
        .collect::<Result<BTreeSet<_>>>()?;
    let stored = contract
        .operations
        .keys()
        .map(|key| {
            let key = schema::parse_operation_key(key)?;
            let (path, _) = crate::openapi::identity::canonical_path_template(&key.path)?;
            Ok(crate::contract::OperationIdentity {
                method: key.method,
                path,
            })
        })
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
        let canonical = format!("{} {}", parsed.method.as_str(), parsed.path);
        if operation_key != &canonical {
            return Err(anyhow!("operation key is not canonical"));
        }
        for (name, auth) in &operation.auth {
            validate_wire_string(name, "auth name", false)?;
            validate_normalized_strings(&auth.scopes, "auth scopes")?;
        }
        for parameter_key in operation.parameters.keys() {
            let parsed = schema::parse_parameter_key(parameter_key)?;
            let canonical = format!("{}:{}", parsed.location.as_str(), parsed.name);
            if parameter_key != &canonical {
                return Err(anyhow!("parameter key is not canonical"));
            }
        }
        if let Some(content) = &operation.request_body {
            validate_content_types(content)?;
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
        for property in schema.properties.keys() {
            validate_wire_string(property, "schema property name", true)?;
        }
    }
    Ok(())
}

fn validate_content_types(content: &BTreeMap<String, String>) -> Result<()> {
    for content_type in content.keys() {
        validate_wire_string(content_type, "media type", false)?;
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod semantic_validation_tests {
    use super::*;

    fn fixture() -> Contract {
        let schema = WireSchema {
            kind: SchemaKind::String,
            nullable: false,
            format: Some("uuid".to_string()),
            enum_values: vec!["a".to_string(), "b".to_string()],
            properties: BTreeMap::from([(
                "value".to_string(),
                WireProperty {
                    required: true,
                    schema: "sha256:schema".to_string(),
                },
            )]),
        };
        Contract {
            operations: BTreeMap::from([(
                "GET /users".to_string(),
                WireOperation {
                    auth: BTreeMap::from([(
                        "bearer".to_string(),
                        WireAuth {
                            kind: AuthSchemeKind::Bearer,
                            scopes: vec!["read".to_string(), "write".to_string()],
                        },
                    )]),
                    parameters: BTreeMap::from([(
                        "query:page".to_string(),
                        WireParameter {
                            required: false,
                            schema: "sha256:schema".to_string(),
                        },
                    )]),
                    request_body: Some(BTreeMap::from([(
                        "application/json".to_string(),
                        "sha256:schema".to_string(),
                    )])),
                    responses: BTreeMap::from([(
                        "200".to_string(),
                        BTreeMap::from([(
                            "application/json".to_string(),
                            "sha256:schema".to_string(),
                        )]),
                    )]),
                },
            )]),
            schemas: BTreeMap::from([("sha256:schema".to_string(), schema)]),
        }
    }

    fn assert_control_rejected(contract: &Contract) {
        let error = validate_contract_semantics(contract)
            .unwrap_err()
            .to_string();
        assert!(error.contains("control character"), "{error}");
        assert!(!error.contains('\u{1b}'), "{error:?}");
    }

    #[test]
    fn rejects_control_characters_from_every_wire_string_category() {
        let mut contract = fixture();
        let operation = contract.operations.get_mut("GET /users").unwrap();
        let auth = operation.auth.remove("bearer").unwrap();
        operation.auth.insert("bearer\u{1b}".to_string(), auth);
        assert_control_rejected(&contract);

        let mut contract = fixture();
        contract
            .operations
            .get_mut("GET /users")
            .unwrap()
            .responses
            .insert("2\u{1b}00".to_string(), BTreeMap::new());
        assert_control_rejected(&contract);

        let mut contract = fixture();
        contract
            .operations
            .get_mut("GET /users")
            .unwrap()
            .request_body
            .as_mut()
            .unwrap()
            .insert(
                "application/\u{1b}json".to_string(),
                "sha256:schema".to_string(),
            );
        assert_control_rejected(&contract);

        let mut contract = fixture();
        contract.schemas.get_mut("sha256:schema").unwrap().format = Some("u\u{1b}uid".to_string());
        assert_control_rejected(&contract);

        let mut contract = fixture();
        contract
            .schemas
            .get_mut("sha256:schema")
            .unwrap()
            .enum_values[0] = "a\u{1b}".to_string();
        assert_control_rejected(&contract);

        let mut contract = fixture();
        let schema = contract.schemas.get_mut("sha256:schema").unwrap();
        let property = schema.properties.remove("value").unwrap();
        schema
            .properties
            .insert("val\u{1b}ue".to_string(), property);
        assert_control_rejected(&contract);
    }

    #[test]
    fn rejects_unsorted_or_duplicate_normalized_arrays() {
        let mut contract = fixture();
        contract
            .operations
            .get_mut("GET /users")
            .unwrap()
            .auth
            .get_mut("bearer")
            .unwrap()
            .scopes = vec!["write".to_string(), "read".to_string()];
        assert!(validate_contract_semantics(&contract)
            .unwrap_err()
            .to_string()
            .contains("sorted and contain no duplicates"));

        let mut contract = fixture();
        contract
            .schemas
            .get_mut("sha256:schema")
            .unwrap()
            .enum_values = vec!["a".to_string(), "a".to_string()];
        assert!(validate_contract_semantics(&contract)
            .unwrap_err()
            .to_string()
            .contains("sorted and contain no duplicates"));
    }
}

#[cfg(test)]
pub(super) fn schema_id_for_test(schema: &WireSchema) -> anyhow::Result<String> {
    canonical::schema_id(schema)
}

#[cfg(test)]
pub(super) fn contract_digest_for_test(extensions: &Extensions) -> anyhow::Result<String> {
    canonical::contract_digest(&Scope::all(), &Contract::default(), extensions)
}

#[cfg(test)]
pub(super) fn intern_and_expand_for_test(
    contract: &crate::contract::ApiContract,
) -> anyhow::Result<(usize, crate::contract::ApiContract)> {
    let wire = schema::intern_contract(contract)?;
    let count = wire.schemas.len();
    let expanded = schema::expand_contract(&wire)?;
    Ok((count, expanded))
}

#[cfg(test)]
pub(super) fn tampered_schema_error_for_test(
    contract: &crate::contract::ApiContract,
) -> anyhow::Result<()> {
    let mut wire = schema::intern_contract(contract)?;
    let (id, mut schema) = wire
        .schemas
        .pop_first()
        .ok_or_else(|| anyhow::anyhow!("test contract has no schemas"))?;
    schema.nullable = !schema.nullable;
    wire.schemas.insert(id, schema);
    schema::validate_schema_table(&wire)
}

#[cfg(test)]
pub(super) fn orphan_schema_error_for_test(
    contract: &crate::contract::ApiContract,
) -> anyhow::Result<()> {
    let mut wire = schema::intern_contract(contract)?;
    let orphan = WireSchema::unknown();
    wire.schemas.insert(canonical::schema_id(&orphan)?, orphan);
    schema::validate_schema_table(&wire)
}

#[cfg(test)]
pub(super) fn forced_collision_error_for_test() -> anyhow::Result<()> {
    schema::forced_collision_error()
}

#[cfg(test)]
pub(super) fn measured_contract_bytes_for_test(
    contract: &crate::contract::ApiContract,
) -> anyhow::Result<u64> {
    let wire = schema::intern_contract(contract)?;
    u64::try_from(contract_yaml(&wire)?.len()).context("contract size overflow")
}

#[cfg(test)]
pub(super) fn tampered_contract_bytes_error_for_test(
    contract: &crate::contract::ApiContract,
) -> anyhow::Result<()> {
    let mut entry = build_declared(contract, Scope::all(), u64::MAX, BTreeMap::new())?;
    entry.contract_bytes += 1;
    validate_declared("test", &entry).map(|_| ())
}

#[cfg(test)]
pub(super) fn tampered_contract_digest_error_for_test(
    contract: &crate::contract::ApiContract,
) -> anyhow::Result<()> {
    let mut entry = build_declared(contract, Scope::all(), u64::MAX, BTreeMap::new())?;
    entry.contract_digest = format!("sha256:{}", "0".repeat(64));
    validate_declared("test", &entry).map(|_| ())
}
