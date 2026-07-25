mod canonical;
mod schema;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::contract::{AuthSchemeKind, SchemaKind};

pub(super) const DEFAULT_MAX_LOCK_BYTES: u64 = 5_242_880;

pub(super) type Extensions = BTreeMap<String, serde_json::Value>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct V3Lock {
    version: u8,
    apis: BTreeMap<String, V3Api>,
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
pub(super) enum Scope {
    All(AllScope),
    Operations(OperationScope),
}

impl Scope {
    pub(super) fn all() -> Self {
        Self::All(AllScope::All)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum AllScope {
    #[serde(rename = "all")]
    All,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OperationScope {
    operations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DeclaredEntry {
    source: String,
    scope: Scope,
    max_lock_bytes: u64,
    contract_bytes: u64,
    contract_digest: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    extensions: Extensions,
    contract: Contract,
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
