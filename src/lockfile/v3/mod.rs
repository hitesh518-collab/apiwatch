mod canonical;
mod schema;

use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

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
#[serde(untagged)]
pub enum Scope {
    All(AllScope),
    Operations(OperationScope),
}

impl Scope {
    pub(super) fn all() -> Self {
        Self::All(AllScope::All)
    }

    pub(super) fn operations(operations: Vec<String>) -> Self {
        Self::Operations(OperationScope { operations })
    }

    pub(super) fn selectors(&self) -> &[String] {
        match self {
            Self::All(_) => &[],
            Self::Operations(scope) => &scope.operations,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllScope {
    #[serde(rename = "all")]
    All,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationScope {
    operations: Vec<String>,
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
    let lock: V3Lock =
        serde_yaml::from_str(contents).context("failed to parse api.lock v3 YAML")?;
    validate_lock(&lock)?;
    Ok(lock)
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
        if previous.as_ref().is_some_and(|value| value >= &key) {
            return Err(anyhow!(
                "operation scope must be sorted and contain no duplicates"
            ));
        }
        previous = Some(key);
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(anyhow!("api name cannot be empty"));
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
