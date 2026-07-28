use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::contract::ApiContract;
use crate::diff::{Change, Severity};
use crate::observed::{apply_map_annotations, merge as merge_shapes, Shape};

mod atomic;
#[doc(hidden)]
pub mod v3;
#[doc(hidden)]
pub mod v4;

pub const DEFAULT_MAX_LOCK_BYTES: u64 = v3::DEFAULT_MAX_LOCK_BYTES;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scope {
    All(AllScope),
    Operations(OperationScope),
}

impl Scope {
    pub(crate) fn all() -> Self {
        Self::All(AllScope::All)
    }

    pub(crate) fn operations(operations: Vec<String>) -> Self {
        Self::Operations(OperationScope { operations })
    }

    pub(crate) fn selectors(&self) -> &[String] {
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

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiLock {
    version: u8,
    #[serde(rename = "apis")]
    legacy_declared: BTreeMap<String, LockedApi>,
    #[serde(skip)]
    declared_v3: BTreeMap<String, v3::DeclaredEntry>,
    #[serde(skip)]
    declared_v4: BTreeMap<String, v4::DeclaredEntry>,
    #[serde(skip)]
    observed: BTreeMap<String, Shape>,
}

#[derive(Deserialize)]
struct LockVersion {
    version: u8,
}

#[derive(Deserialize)]
struct V2Lock {
    version: u8,
    apis: BTreeMap<String, V2LockedApi>,
}

#[derive(Deserialize)]
struct V2LockedApi {
    provenance: String,
    source: Option<String>,
    operations: Option<Vec<LockedOperation>>,
    shape: Option<Shape>,
}

#[derive(Serialize)]
struct V2RenderedLock<'a> {
    version: u8,
    apis: BTreeMap<&'a String, V2RenderedApi<'a>>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum V2RenderedApi<'a> {
    Declared {
        provenance: &'static str,
        source: &'a str,
        operations: &'a [LockedOperation],
    },
    Observed {
        provenance: &'static str,
        shape: &'a Shape,
    },
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct LockedApi {
    source: String,
    operations: Vec<LockedOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LockedOperation {
    method: String,
    path: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct VerifyTarget {
    name: String,
    kind: VerifyTargetKind,
}

#[derive(Debug, PartialEq, Eq)]
pub enum VerifyTargetKind {
    LegacyDeclared {
        operations: BTreeSet<LockedOperation>,
    },
    Declared {
        contract: ApiContract,
        scope: Scope,
        coverage: DeclaredCoverage,
    },
    Observed {
        shape: Shape,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredCoverage {
    PartialV3,
    FullV4,
}

impl VerifyTarget {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn observed_shape(&self) -> Option<&Shape> {
        match &self.kind {
            VerifyTargetKind::Observed { shape } => Some(shape),
            _ => None,
        }
    }

    pub fn kind(&self) -> &VerifyTargetKind {
        &self.kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyChangeKind {
    Removed,
    Added,
}

impl VerifyChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Removed => "REMOVED",
            Self::Added => "ADDED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyChange {
    pub kind: VerifyChangeKind,
    pub method: String,
    pub path: String,
}

pub fn from_contract(name: &str, contract: &ApiContract) -> Result<ApiLock> {
    let name = normalized_name(name)?;

    let operations = contract
        .operations
        .values()
        .map(|operation| LockedOperation {
            method: operation.key.method.as_str().to_string(),
            path: operation.key.path.clone(),
        })
        .collect();

    let mut apis = BTreeMap::new();
    apis.insert(
        name.to_string(),
        LockedApi {
            source: "openapi".to_string(),
            operations,
        },
    );

    Ok(ApiLock {
        version: 1,
        legacy_declared: apis,
        declared_v3: BTreeMap::new(),
        declared_v4: BTreeMap::new(),
        observed: BTreeMap::new(),
    })
}

pub fn render(lock: &ApiLock) -> Result<String> {
    if lock.version == 1 {
        return serde_yaml::to_string(lock).context("failed to serialize lockfile");
    }
    if lock.version == 3 {
        return v3::render(&v3::V3Lock::from_parts(
            lock.declared_v3.clone(),
            lock.observed.clone(),
        ));
    }
    if lock.version == 4 {
        return v4::render(&v4::V4Lock::from_parts(
            lock.declared_v4.clone(),
            lock.observed.clone(),
        ));
    }

    let mut apis = BTreeMap::new();
    for (name, api) in &lock.legacy_declared {
        apis.insert(
            name,
            V2RenderedApi::Declared {
                provenance: "declared",
                source: &api.source,
                operations: &api.operations,
            },
        );
    }
    for (name, shape) in &lock.observed {
        apis.insert(
            name,
            V2RenderedApi::Observed {
                provenance: "observed",
                shape,
            },
        );
    }

    serde_yaml::to_string(&V2RenderedLock { version: 2, apis })
        .context("failed to serialize lockfile")
}

pub fn load(path: &Path) -> Result<ApiLock> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read api.lock {}", path.display()))?;
    let header: LockVersion =
        serde_yaml::from_str(&contents).context("failed to parse api.lock YAML")?;

    match header.version {
        1 => serde_yaml::from_str(&contents).context("failed to parse api.lock YAML"),
        2 => load_v2(&contents),
        3 => {
            let (declared, observed) = v3::load(&contents)?.into_parts();
            Ok(ApiLock {
                version: 3,
                legacy_declared: BTreeMap::new(),
                declared_v3: declared,
                declared_v4: BTreeMap::new(),
                observed,
            })
        }
        4 => {
            let (declared_v4, observed) = v4::load(&contents)?.into_parts();
            Ok(ApiLock {
                version: 4,
                legacy_declared: BTreeMap::new(),
                declared_v3: BTreeMap::new(),
                declared_v4,
                observed,
            })
        }
        version => Err(anyhow!("unsupported api.lock version {version}")),
    }
}

pub fn new_v3(name: &str, entry: v3::DeclaredEntry) -> Result<ApiLock> {
    let name = normalized_name(name)?.to_owned();
    let lock = ApiLock {
        version: 3,
        legacy_declared: BTreeMap::new(),
        declared_v3: BTreeMap::from([(name, entry)]),
        declared_v4: BTreeMap::new(),
        observed: BTreeMap::new(),
    };
    v3::render(&v3::V3Lock::from_parts(
        lock.declared_v3.clone(),
        BTreeMap::new(),
    ))?;
    Ok(lock)
}

pub fn scope_from_selectors(selectors: &[String]) -> Result<Scope> {
    if selectors.is_empty() {
        return Ok(Scope::all());
    }
    let mut operations = BTreeSet::new();
    for selector in selectors {
        let key = crate::lock_size::parse_operation_selector(selector)?;
        if !operations.insert(key) {
            return Err(anyhow!("duplicate operation selector"));
        }
    }
    Ok(Scope::operations(
        operations
            .into_iter()
            .map(|key| format!("{} {}", key.method.as_str(), key.path))
            .collect(),
    ))
}

pub fn build_v3_declared(
    contract: &ApiContract,
    scope: Scope,
    max_lock_bytes: u64,
) -> Result<v3::DeclaredEntry> {
    v3::build_declared(contract, scope, max_lock_bytes, BTreeMap::new())
}

pub fn new_v4(name: &str, entry: v4::DeclaredEntry) -> Result<ApiLock> {
    let name = normalized_name(name)?.to_owned();
    let lock = ApiLock {
        version: 4,
        legacy_declared: BTreeMap::new(),
        declared_v3: BTreeMap::new(),
        declared_v4: BTreeMap::from([(name, entry)]),
        observed: BTreeMap::new(),
    };
    v4::render(&v4::V4Lock::from_parts(
        lock.declared_v4.clone(),
        BTreeMap::new(),
    ))?;
    Ok(lock)
}

pub fn build_v4_declared(
    contract: &ApiContract,
    scope: Scope,
    max_lock_bytes: u64,
) -> Result<v4::DeclaredEntry> {
    v4::build_declared(contract, scope, max_lock_bytes, BTreeMap::new())
}

pub fn atomic_write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic::write_new(path, bytes)
}

pub fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic::replace(path, bytes)
}

pub fn replace_declared(
    mut lock: ApiLock,
    name: &str,
    entry: v3::DeclaredEntry,
) -> Result<ApiLock> {
    let name = normalized_name(name)?.to_owned();
    if lock.observed.contains_key(&name) {
        return Err(anyhow!(
            "api {name} is observed and cannot be replaced as declared"
        ));
    }
    if lock.version < 3 {
        if !lock.legacy_declared.contains_key(&name) {
            return Err(anyhow!("legacy declared api {name} not found"));
        }
        let remaining: Vec<_> = lock
            .legacy_declared
            .keys()
            .filter(|candidate| candidate.as_str() != name)
            .cloned()
            .collect();
        if !remaining.is_empty() {
            let remaining = remaining
                .iter()
                .map(|name| sanitized(name))
                .collect::<Vec<_>>();
            return Err(anyhow!(
                "cannot migrate api.lock to v3; migration requires original sources for: {}",
                remaining.join(", ")
            ));
        }
        lock.legacy_declared.clear();
    }
    lock.version = 3;
    lock.declared_v3.insert(name, entry);
    v3::render(&v3::V3Lock::from_parts(
        lock.declared_v3.clone(),
        lock.observed.clone(),
    ))?;
    Ok(lock)
}

pub fn replace_declared_v4(
    mut lock: ApiLock,
    name: &str,
    entry: v4::DeclaredEntry,
) -> Result<ApiLock> {
    let name = normalized_name(name)?.to_owned();
    if lock.observed.contains_key(&name) {
        return Err(anyhow!(
            "api {name} is observed and cannot be replaced as declared"
        ));
    }
    if lock.version < 4 {
        let named_old_entry =
            lock.legacy_declared.contains_key(&name) || lock.declared_v3.contains_key(&name);
        if !named_old_entry {
            return Err(anyhow!("declared api {name} not found"));
        }
        let mut remaining = lock
            .legacy_declared
            .keys()
            .chain(lock.declared_v3.keys())
            .filter(|candidate| candidate.as_str() != name)
            .map(|name| sanitized(name))
            .collect::<Vec<_>>();
        remaining.sort();
        if !remaining.is_empty() {
            return Err(anyhow!(
                "cannot migrate api.lock to v4; migration requires original sources for: {}",
                remaining.join(", ")
            ));
        }
        lock.legacy_declared.clear();
        lock.declared_v3.clear();
        lock.version = 4;
    }
    lock.declared_v4.insert(name, entry);
    v4::render(&v4::V4Lock::from_parts(
        lock.declared_v4.clone(),
        lock.observed.clone(),
    ))?;
    Ok(lock)
}

pub fn record_observed(
    lock: &mut ApiLock,
    name: &str,
    incoming: Shape,
    merge_existing: bool,
    map_paths: &[String],
) -> Result<()> {
    let name = normalized_name(name)?;
    if lock.legacy_declared.contains_key(name)
        || lock.declared_v3.contains_key(name)
        || lock.declared_v4.contains_key(name)
    {
        return Err(anyhow!(
            "api {name} is declared and cannot be recorded as observed"
        ));
    }

    let mut incoming = incoming;
    apply_map_annotations(&mut incoming, map_paths)?;

    match lock.observed.get(name) {
        Some(existing) if merge_existing => {
            let mut existing = existing.clone();
            apply_map_annotations(&mut existing, map_paths)?;
            merge_shapes(&mut existing, &incoming);
            lock.observed.insert(name.to_string(), existing);
        }
        Some(_) => return Err(anyhow!("api {name} already exists; use --merge")),
        None if merge_existing => return Err(anyhow!("observed api {name} was not found")),
        None => {
            lock.observed.insert(name.to_string(), incoming);
        }
    }

    if lock.version < 3 {
        lock.version = 2;
    }
    Ok(())
}

pub fn load_or_create_for_record(path: &Path) -> Result<ApiLock> {
    if path.exists() {
        load(path)
    } else {
        Ok(ApiLock {
            version: 2,
            legacy_declared: BTreeMap::new(),
            declared_v3: BTreeMap::new(),
            declared_v4: BTreeMap::new(),
            observed: BTreeMap::new(),
        })
    }
}

fn load_v2(contents: &str) -> Result<ApiLock> {
    let raw: V2Lock = serde_yaml::from_str(contents).context("failed to parse api.lock YAML")?;
    if raw.version != 2 {
        return Err(anyhow!("unsupported api.lock version {}", raw.version));
    }

    let mut apis = BTreeMap::new();
    let mut observed = BTreeMap::new();
    for (name, api) in raw.apis {
        match api.provenance.as_str() {
            "declared" => {
                let source = api
                    .source
                    .ok_or_else(|| anyhow!("declared api {name} is missing source"))?;
                let operations = api
                    .operations
                    .ok_or_else(|| anyhow!("declared api {name} is missing operations"))?;
                apis.insert(name, LockedApi { source, operations });
            }
            "observed" => {
                let shape = api
                    .shape
                    .ok_or_else(|| anyhow!("observed api {name} is missing shape"))?;
                observed.insert(name, shape);
            }
            provenance => return Err(anyhow!("unsupported api.lock provenance {provenance}")),
        }
    }

    Ok(ApiLock {
        version: 2,
        legacy_declared: apis,
        declared_v3: BTreeMap::new(),
        declared_v4: BTreeMap::new(),
        observed,
    })
}

pub fn select_verify_target(lock: &ApiLock, name: &str) -> Result<VerifyTarget> {
    let name = normalized_name(name)?;
    if let Some(shape) = lock.observed.get(name) {
        return Ok(VerifyTarget {
            name: name.to_string(),
            kind: VerifyTargetKind::Observed {
                shape: shape.clone(),
            },
        });
    }
    if let Some(entry) = lock.declared_v4.get(name) {
        return Ok(VerifyTarget {
            name: name.to_string(),
            kind: VerifyTargetKind::Declared {
                contract: v4::validate_declared(name, entry)?,
                scope: entry.scope().clone(),
                coverage: DeclaredCoverage::FullV4,
            },
        });
    }
    if let Some(entry) = lock.declared_v3.get(name) {
        return Ok(VerifyTarget {
            name: name.to_string(),
            kind: VerifyTargetKind::Declared {
                contract: v3::validate_declared(name, entry)?,
                scope: entry.scope().clone(),
                coverage: DeclaredCoverage::PartialV3,
            },
        });
    }
    let api = lock
        .legacy_declared
        .get(name)
        .ok_or_else(|| anyhow!("api {name} not found in lockfile"))?;

    if api.source.chars().any(char::is_control) {
        return Err(anyhow!("api.lock source contains a control character"));
    }

    if api.source != "openapi" {
        return Err(anyhow!("unsupported api.lock source {}", api.source));
    }

    let operations = api
        .operations
        .iter()
        .enumerate()
        .map(|(index, operation)| {
            normalized_locked_operation(operation)
                .with_context(|| format!("invalid locked operation {}", index + 1))
        })
        .collect::<Result<_>>()?;

    Ok(VerifyTarget {
        name: name.to_string(),
        kind: VerifyTargetKind::LegacyDeclared { operations },
    })
}

pub fn compare_verify_target(target: &VerifyTarget, current: &ApiContract) -> Result<Vec<Change>> {
    let VerifyTargetKind::LegacyDeclared {
        operations: target_operations,
    } = &target.kind
    else {
        return Ok(Vec::new());
    };
    let current_operations: BTreeSet<_> = current
        .operations
        .keys()
        .map(|key| LockedOperation {
            method: key.method.as_str().to_string(),
            path: key.path.clone(),
        })
        .collect();
    let mut changes = Vec::new();

    for operation in target_operations.difference(&current_operations) {
        changes.push(Change {
            severity: Severity::Breaking,
            operation: operation_key_from_locked(operation)?,
            message: "endpoint removed".to_string(),
        });
    }

    for operation in current_operations.difference(target_operations) {
        changes.push(Change {
            severity: Severity::Warning,
            operation: operation_key_from_locked(operation)?,
            message: "endpoint added outside route-only lock".to_string(),
        });
    }

    Ok(changes)
}

pub fn scope_current_for_verify(current: &ApiContract, scope: &Scope) -> Result<ApiContract> {
    crate::lock_size::scope_current_for_verify(current, scope.selectors())
}

fn normalized_name(name: &str) -> Result<&str> {
    if name.chars().any(char::is_control) {
        return Err(anyhow!("api name contains a control character"));
    }
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("api name cannot be empty"));
    }

    Ok(name)
}

fn sanitized(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

fn normalized_locked_operation(operation: &LockedOperation) -> Result<LockedOperation> {
    let method = operation.method.to_ascii_uppercase();
    if method.chars().any(char::is_control) {
        return Err(anyhow!(
            "locked operation method contains a control character"
        ));
    }

    if !matches!(
        method.as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS" | "HEAD" | "TRACE"
    ) {
        return Err(anyhow!("unsupported locked operation method"));
    }

    if operation.path.is_empty() {
        return Err(anyhow!("locked operation path cannot be empty"));
    }

    if !operation.path.starts_with('/') {
        return Err(anyhow!("locked operation path must start with /"));
    }

    if operation.path.chars().any(char::is_control) {
        return Err(anyhow!(
            "locked operation path contains a control character"
        ));
    }

    Ok(LockedOperation {
        method,
        path: operation.path.clone(),
    })
}

fn operation_key_from_locked(operation: &LockedOperation) -> Result<crate::contract::OperationKey> {
    let operation = normalized_locked_operation(operation)?;
    let method = match operation.method.as_str() {
        "GET" => crate::contract::HttpMethod::Get,
        "POST" => crate::contract::HttpMethod::Post,
        "PUT" => crate::contract::HttpMethod::Put,
        "PATCH" => crate::contract::HttpMethod::Patch,
        "DELETE" => crate::contract::HttpMethod::Delete,
        "OPTIONS" => crate::contract::HttpMethod::Options,
        "HEAD" => crate::contract::HttpMethod::Head,
        "TRACE" => crate::contract::HttpMethod::Trace,
        _ => unreachable!("normalized locked operation validates the method"),
    };
    Ok(crate::contract::OperationKey {
        method,
        path: operation.path,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::contract::Schema;

    fn canonical_lines(value: &str) -> String {
        value.replace("\r\n", "\n")
    }

    fn with_unknown_additional_properties(contract: &mut ApiContract) {
        for operation in contract.operations.values_mut() {
            for parameter in operation.parameters.values_mut() {
                mark_schema_additional_properties_unknown(&mut parameter.schema);
            }
            if let Some(request_body) = &mut operation.request_body {
                for schema in request_body.content.values_mut() {
                    mark_schema_additional_properties_unknown(schema);
                }
            }
            for response in operation.responses.values_mut() {
                for schema in response.content.values_mut() {
                    mark_schema_additional_properties_unknown(schema);
                }
            }
        }
    }

    fn mark_schema_additional_properties_unknown(schema: &mut Schema) {
        schema.additional_properties = crate::contract::AdditionalProperties::Unknown;
        for property in schema.properties.values_mut() {
            mark_schema_additional_properties_unknown(&mut property.schema);
        }
    }

    fn v3_declared_fixture() -> v3::DeclaredEntry {
        let contract =
            crate::openapi::load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml"))
                .expect("fixture should load");
        v3::build_declared(
            &contract,
            v3::Scope::all(),
            v3::DEFAULT_MAX_LOCK_BYTES,
            BTreeMap::new(),
        )
        .expect("v3 fixture should build")
    }

    #[test]
    fn replaces_the_sole_legacy_declared_entry_and_preserves_observed() {
        let lock = load(Path::new("testdata/lock/v2_declared_observed.lock"))
            .expect("v2 fixture should load");
        let entry = v3_declared_fixture();

        let migrated =
            replace_declared(lock, "users", entry.clone()).expect("migration should succeed");
        let rendered = render(&migrated).expect("v3 lock should render");

        assert!(rendered.starts_with("version: 3\n"));
        assert!(rendered.contains("provenance: declared"));
        assert!(rendered.contains("provenance: observed"));
        assert_eq!(migrated.declared_v3["users"], entry);
    }

    #[test]
    fn refuses_partial_migration_of_multiple_legacy_entries() {
        let lock = load(Path::new("testdata/lock/v2_multiple_declared.lock"))
            .expect("v2 fixture should load");

        let error = replace_declared(lock, "users", v3_declared_fixture())
            .expect_err("partial migration must fail");

        assert!(error.to_string().contains("requires original sources"));
        assert!(error.to_string().contains("payments"));
    }

    #[test]
    fn migration_diagnostics_do_not_echo_control_characters_from_legacy_names() {
        let lock = ApiLock {
            version: 2,
            legacy_declared: BTreeMap::from([
                (
                    "users".to_string(),
                    LockedApi {
                        source: "openapi".to_string(),
                        operations: Vec::new(),
                    },
                ),
                (
                    "payments\u{1b}[31m".to_string(),
                    LockedApi {
                        source: "openapi".to_string(),
                        operations: Vec::new(),
                    },
                ),
            ]),
            declared_v3: BTreeMap::new(),
            declared_v4: BTreeMap::new(),
            observed: BTreeMap::new(),
        };

        let error = replace_declared(lock, "users", v3_declared_fixture())
            .expect_err("ambiguous migration should fail safely");

        assert!(!error.to_string().contains('\u{1b}'));
    }

    #[test]
    fn refuses_to_replace_an_observed_name() {
        let lock = load(Path::new("testdata/lock/v2_declared_observed.lock"))
            .expect("v2 fixture should load");

        let error = replace_declared(lock, "portfolio", v3_declared_fixture())
            .expect_err("observed entry must not become declared");

        assert!(error.to_string().contains("is observed"));
    }

    #[test]
    fn top_level_load_and_render_round_trip_v3() {
        let expected = fs::read_to_string("testdata/lock/v3_private.lock")
            .expect("v3 fixture should be readable");
        let lock =
            load(Path::new("testdata/lock/v3_private.lock")).expect("v3 fixture should load");

        assert_eq!(
            render(&lock).expect("v3 fixture should render"),
            canonical_lines(&expected)
        );
    }

    #[test]
    fn recording_an_observed_entry_preserves_v3() {
        let mut lock =
            load(Path::new("testdata/lock/v3_private.lock")).expect("v3 fixture should load");

        record_observed(&mut lock, "portfolio", Shape::String, false, &[])
            .expect("observed entry should record");
        let rendered = render(&lock).expect("v3 lock should render");

        assert!(rendered.starts_with("version: 3\n"));
        assert!(rendered.contains("provenance: declared"));
        assert!(rendered.contains("provenance: observed"));
    }

    #[test]
    fn compare_verify_target_reports_removed_before_added_in_order() {
        let lock = ApiLock {
            version: 1,
            legacy_declared: BTreeMap::from([(
                "users".to_string(),
                LockedApi {
                    source: "openapi".to_string(),
                    operations: vec![
                        LockedOperation {
                            method: "GET".to_string(),
                            path: "/zeta".to_string(),
                        },
                        LockedOperation {
                            method: "GET".to_string(),
                            path: "/users".to_string(),
                        },
                    ],
                },
            )]),
            declared_v3: BTreeMap::new(),
            declared_v4: BTreeMap::new(),
            observed: BTreeMap::new(),
        };
        let current =
            crate::openapi::load_contract(Path::new("testdata/openapi/verify_current.yaml"))
                .expect("fixture should load");

        let target = select_verify_target(&lock, "users").expect("target should select");

        assert_eq!(target.name(), "users");
        assert_eq!(
            compare_verify_target(&target, &current).expect("comparison should succeed"),
            vec![
                Change {
                    severity: Severity::Breaking,
                    operation: operation_key_from_locked(&LockedOperation {
                        method: "GET".to_string(),
                        path: "/users".to_string()
                    })
                    .unwrap(),
                    message: "endpoint removed".to_string(),
                },
                Change {
                    severity: Severity::Breaking,
                    operation: operation_key_from_locked(&LockedOperation {
                        method: "GET".to_string(),
                        path: "/zeta".to_string()
                    })
                    .unwrap(),
                    message: "endpoint removed".to_string(),
                },
                Change {
                    severity: Severity::Warning,
                    operation: operation_key_from_locked(&LockedOperation {
                        method: "POST".to_string(),
                        path: "/users".to_string()
                    })
                    .unwrap(),
                    message: "endpoint added outside route-only lock".to_string(),
                },
                Change {
                    severity: Severity::Warning,
                    operation: operation_key_from_locked(&LockedOperation {
                        method: "POST".to_string(),
                        path: "/zeta".to_string()
                    })
                    .unwrap(),
                    message: "endpoint added outside route-only lock".to_string(),
                },
            ]
        );
    }

    #[test]
    fn select_verify_target_normalizes_locked_method_case() {
        let lock = ApiLock {
            version: 1,
            legacy_declared: BTreeMap::from([(
                "users".to_string(),
                LockedApi {
                    source: "openapi".to_string(),
                    operations: vec![LockedOperation {
                        method: "get".to_string(),
                        path: "/users".to_string(),
                    }],
                },
            )]),
            declared_v3: BTreeMap::new(),
            declared_v4: BTreeMap::new(),
            observed: BTreeMap::new(),
        };
        let current =
            crate::openapi::load_contract(Path::new("testdata/openapi/lock_ordering.yaml"))
                .expect("fixture should load");

        let target = select_verify_target(&lock, "users").expect("target should select");

        assert_eq!(
            compare_verify_target(&target, &current).expect("comparison should succeed"),
            vec![Change {
                severity: Severity::Warning,
                operation: operation_key_from_locked(&LockedOperation {
                    method: "POST".to_string(),
                    path: "/users".to_string()
                })
                .unwrap(),
                message: "endpoint added outside route-only lock".to_string(),
            }]
        );
    }

    #[test]
    fn select_verify_target_rejects_an_unsupported_locked_method() {
        let lock = ApiLock {
            version: 1,
            legacy_declared: BTreeMap::from([(
                "users".to_string(),
                LockedApi {
                    source: "openapi".to_string(),
                    operations: vec![LockedOperation {
                        method: "BOGUS".to_string(),
                        path: "/users".to_string(),
                    }],
                },
            )]),
            declared_v3: BTreeMap::new(),
            declared_v4: BTreeMap::new(),
            observed: BTreeMap::new(),
        };

        let error = select_verify_target(&lock, "users")
            .expect_err("unsupported locked method should be rejected");

        assert!(error.chain().any(|cause| cause
            .to_string()
            .contains("unsupported locked operation method")));
    }

    #[test]
    fn select_verify_target_rejects_a_locked_path_with_a_control_character() {
        let lock = ApiLock {
            version: 1,
            legacy_declared: BTreeMap::from([(
                "users".to_string(),
                LockedApi {
                    source: "openapi".to_string(),
                    operations: vec![LockedOperation {
                        method: "GET".to_string(),
                        path: "/users\u{0001}".to_string(),
                    }],
                },
            )]),
            declared_v3: BTreeMap::new(),
            declared_v4: BTreeMap::new(),
            observed: BTreeMap::new(),
        };

        let error = select_verify_target(&lock, "users")
            .expect_err("locked path with a control character should be rejected");

        assert!(error.chain().any(|cause| cause
            .to_string()
            .contains("locked operation path contains a control character")));
        assert!(!error
            .chain()
            .any(|cause| cause.to_string().contains('\u{0001}')));
    }

    #[test]
    fn load_rejects_an_invalid_v4_lockfile() {
        let error = load(Path::new("testdata/lock/verify_unsupported_version.lock"))
            .expect_err("invalid version 4 lockfile should be rejected");

        assert!(error
            .to_string()
            .contains("failed to parse api.lock v4 YAML"));
    }

    #[test]
    fn recording_into_v1_preserves_declared_operations_and_renders_v2() {
        let mut lock =
            load(Path::new("testdata/lock/verify_users.lock")).expect("v1 lock should load");
        let shape = crate::observed::infer(&serde_json::json!({
            "id": 1,
            "token": "super-secret-token"
        }));

        record_observed(&mut lock, "portfolio", shape, false, &[])
            .expect("new observed entry should be recorded");
        let rendered = render(&lock).expect("v2 lock should render");

        assert!(rendered.starts_with("version: 2\n"));
        assert!(rendered.contains("provenance: declared"));
        assert!(rendered.contains("provenance: observed"));
        assert!(rendered.contains("path: /users"));
        assert!(!rendered.contains("super-secret-token"));
    }

    #[test]
    fn invalid_map_annotation_leaves_an_existing_observed_entry_unchanged() {
        let mut lock = ApiLock {
            version: 2,
            legacy_declared: BTreeMap::new(),
            declared_v3: BTreeMap::new(),
            declared_v4: BTreeMap::new(),
            observed: BTreeMap::new(),
        };
        record_observed(
            &mut lock,
            "portfolio",
            crate::observed::infer(&serde_json::json!({"by_broker": {"acme": 1}})),
            false,
            &[],
        )
        .expect("initial observed entry should be recorded");
        let before = render(&lock).expect("lock should serialize");

        let error = record_observed(
            &mut lock,
            "portfolio",
            crate::observed::infer(&serde_json::json!({"by_broker": {"acme": 2}})),
            true,
            &["$.by-broker".to_owned()],
        )
        .expect_err("invalid annotation should fail");

        assert!(error.to_string().contains("invalid map annotation"));
        assert_eq!(render(&lock).expect("lock should serialize"), before);
    }

    #[test]
    fn v3_schema_ids_are_domain_separated_and_stable() {
        let schema = super::v3::WireSchema::unknown();

        let first = super::v3::schema_id_for_test(&schema).expect("schema should hash");
        let second = super::v3::schema_id_for_test(&schema).expect("schema should hash");

        assert_eq!(first, second);
        assert!(first.starts_with("sha256:"));
        assert_eq!(first.len(), 71);
    }

    #[test]
    fn v3_extension_order_does_not_change_contract_digest() {
        let left = BTreeMap::from([
            ("x-b".to_string(), serde_json::json!({"z": 2, "a": 1})),
            ("x-a".to_string(), serde_json::json!(true)),
        ]);
        let right = BTreeMap::from([
            ("x-a".to_string(), serde_json::json!(true)),
            ("x-b".to_string(), serde_json::json!({"a": 1, "z": 2})),
        ]);

        assert_eq!(
            super::v3::contract_digest_for_test(&left).expect("extensions should hash"),
            super::v3::contract_digest_for_test(&right).expect("extensions should hash")
        );
    }

    #[test]
    fn v3_rejects_extension_keys_without_x_prefix() {
        let extensions = BTreeMap::from([("vendor".to_string(), serde_json::json!(true))]);

        let error = super::v3::contract_digest_for_test(&extensions)
            .expect_err("non-prefixed extension should fail");

        assert!(error
            .to_string()
            .contains("extension key must start with x-"));
    }

    #[test]
    fn v3_rejects_control_characters_in_extension_strings_without_echoing_them() {
        let extensions = BTreeMap::from([(
            "x-vendor".to_string(),
            serde_json::json!({"nested": "unsafe\u{1b}"}),
        )]);

        let error = super::v3::contract_digest_for_test(&extensions)
            .expect_err("extension control characters should fail")
            .to_string();

        assert!(error.contains("control character"), "{error}");
        assert!(!error.contains('\u{1b}'), "{error:?}");
    }

    #[test]
    fn v3_interns_repeated_schemas_and_expands_the_original_contract() {
        use crate::contract::{
            AdditionalProperties, HttpMethod, Operation, OperationIdentity, OperationKey, Response,
            Schema, SchemaKind,
        };

        let schema = Schema {
            kind: SchemaKind::String,
            nullable: false,
            format: Some("uuid".to_string()),
            enum_values: Vec::new(),
            properties: BTreeMap::new(),
            additional_properties: AdditionalProperties::Unknown,
            branches: Vec::new(),
        };
        let operation = |path: &str| {
            (
                OperationIdentity {
                    method: HttpMethod::Get,
                    path: path.to_string(),
                },
                Operation {
                    key: OperationKey {
                        method: HttpMethod::Get,
                        path: path.to_string(),
                    },
                    auth: BTreeMap::new(),
                    servers: None,
                    parameters: BTreeMap::new(),
                    request_body: None,
                    responses: BTreeMap::from([(
                        "200".to_string(),
                        Response {
                            content: BTreeMap::from([(
                                "application/json".to_string(),
                                schema.clone(),
                            )]),
                        },
                    )]),
                },
            )
        };
        let contract = ApiContract {
            operations: BTreeMap::from([operation("/accounts"), operation("/users")]),
        };

        let (schema_count, expanded) =
            super::v3::intern_and_expand_for_test(&contract).expect("contract should round trip");

        assert_eq!(schema_count, 1);
        assert_eq!(expanded, contract);
    }

    #[test]
    fn v3_expands_synthetic_allof_with_outer_nullability() {
        use crate::contract::{
            AdditionalProperties, HttpMethod, Operation, OperationIdentity, OperationKey, Property,
            Response, SchemaKind,
        };

        let branch = Schema {
            kind: SchemaKind::Object,
            nullable: true,
            format: None,
            enum_values: Vec::new(),
            properties: BTreeMap::new(),
            additional_properties: AdditionalProperties::Unknown,
            branches: Vec::new(),
        };
        let schema = Schema {
            kind: SchemaKind::AllOf,
            nullable: false,
            format: None,
            enum_values: Vec::new(),
            properties: BTreeMap::from([(
                "allOf[0]".to_string(),
                Property {
                    required: true,
                    schema: Box::new(branch),
                },
            )]),
            additional_properties: AdditionalProperties::Unknown,
            branches: Vec::new(),
        };
        let contract = ApiContract {
            operations: BTreeMap::from([(
                OperationIdentity {
                    method: HttpMethod::Get,
                    path: "/allof".to_string(),
                },
                Operation {
                    key: OperationKey {
                        method: HttpMethod::Get,
                        path: "/allof".to_string(),
                    },
                    auth: BTreeMap::new(),
                    servers: None,
                    parameters: BTreeMap::new(),
                    request_body: None,
                    responses: BTreeMap::from([(
                        "200".to_string(),
                        Response {
                            content: BTreeMap::from([("application/json".to_string(), schema)]),
                        },
                    )]),
                },
            )]),
        };

        let (_, expanded) =
            super::v3::intern_and_expand_for_test(&contract).expect("legacy v3 should expand");
        let expanded = &expanded.operations.values().next().unwrap().responses["200"].content
            ["application/json"];

        assert!(
            !expanded.nullable,
            "outer allOf nullability must constrain branches"
        );
    }

    #[test]
    fn v4_round_trips_schema_valued_additional_properties() {
        let source = crate::openapi::load_contract(Path::new(
            "testdata/openapi/phase2_d04_additional_properties_old.yaml",
        ))
        .expect("fixture should load");
        let entry = build_v4_declared(&source, Scope::all(), v4::DEFAULT_MAX_LOCK_BYTES)
            .expect("v4 entry should build");
        let lock = new_v4("d04", entry).expect("v4 lock should build");
        let rendered = render(&lock).expect("v4 lock should render");

        assert!(
            rendered.contains("additional_properties:\n            kind: schema"),
            "schema-valued additionalProperties must be represented on the v4 wire"
        );

        let path = std::env::temp_dir().join(format!("apiwatch-d04-{}.lock", std::process::id()));
        fs::write(&path, &rendered).expect("v4 lock should write");
        let parsed = load(&path).expect("v4 lock should load");
        fs::remove_file(path).ok();
        let target = select_verify_target(&parsed, "d04").expect("target should select");

        assert_eq!(
            target.kind(),
            &VerifyTargetKind::Declared {
                contract: source,
                scope: Scope::all(),
                coverage: DeclaredCoverage::FullV4,
            }
        );
    }

    #[test]
    fn v4_rejects_a_tampered_schema_valued_additional_properties_reference() {
        let source = crate::openapi::load_contract(Path::new(
            "testdata/openapi/phase2_d04_additional_properties_old.yaml",
        ))
        .expect("fixture should load");
        let entry = build_v4_declared(&source, Scope::all(), v4::DEFAULT_MAX_LOCK_BYTES)
            .expect("v4 entry should build");
        let rendered = render(&new_v4("d04", entry).expect("v4 lock should build"))
            .expect("v4 lock should render");
        let marker = "additional_properties:\n            kind: schema\n            schema: ";
        assert!(
            rendered.contains(marker),
            "schema-valued additionalProperties must be represented on the v4 wire"
        );
        let tampered = rendered.replacen(marker, &format!("{marker}sha256:tampered"), 1);
        let path =
            std::env::temp_dir().join(format!("apiwatch-d04-tampered-{}.lock", std::process::id()));
        fs::write(&path, tampered).expect("tampered v4 lock should write");
        let error = load(&path).expect_err("tampered nested schema reference should fail");
        fs::remove_file(path).ok();

        assert!(error
            .chain()
            .any(|cause| cause.to_string().contains("schema digest mismatch")));
    }

    #[test]
    fn v4_rejects_unknown_fields_in_additional_properties_policies() {
        let source = crate::openapi::load_contract(Path::new(
            "testdata/openapi/phase2_d04_additional_properties_old.yaml",
        ))
        .expect("fixture should load");
        let entry = build_v4_declared(&source, Scope::all(), v4::DEFAULT_MAX_LOCK_BYTES)
            .expect("v4 entry should build");
        let rendered = render(&new_v4("d04", entry).expect("v4 lock should build"))
            .expect("v4 lock should render");
        for (index, (marker, replacement)) in [
            (
                "additional_properties:\n            kind: any",
                "additional_properties:\n            kind: any\n            unexpected_field: accepted",
            ),
            (
                "additional_properties:\n            kind: schema\n            schema: ",
                "additional_properties:\n            kind: schema\n            unexpected_field: accepted\n            schema: ",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            assert!(rendered.contains(marker), "fixture should include {index} policy");
            let tampered = rendered.replacen(marker, replacement, 1);
            let path = std::env::temp_dir().join(format!(
                "apiwatch-d04-policy-field-{}-{index}.lock",
                std::process::id()
            ));
            fs::write(&path, tampered).expect("tampered v4 lock should write");
            let error = load(&path).expect_err("unknown policy fields should fail");
            fs::remove_file(path).ok();

            assert!(error
                .to_string()
                .contains("unknown field in additionalProperties policy"));
        }
    }

    #[test]
    fn v3_rejects_a_tampered_schema_digest() {
        let contract =
            crate::openapi::load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml"))
                .expect("fixture should load");

        let error = super::v3::tampered_schema_error_for_test(&contract)
            .expect_err("tampered schema ID should fail");

        assert!(error.to_string().contains("schema digest mismatch"));
    }

    #[test]
    fn v3_rejects_an_orphan_schema() {
        let contract =
            crate::openapi::load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml"))
                .expect("fixture should load");

        let error = super::v3::orphan_schema_error_for_test(&contract)
            .expect_err("orphan schema should fail");

        assert!(error.to_string().contains("orphan schema"));
    }

    #[test]
    fn v3_rejects_a_forced_schema_digest_collision() {
        let error = super::v3::forced_collision_error_for_test()
            .expect_err("different schemas with one digest should fail");

        assert!(error.to_string().contains("schema digest collision"));
    }

    #[test]
    fn production_v3_writer_excludes_privacy_sentinels() {
        let source =
            crate::openapi::load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml"))
                .expect("fixture should load");
        let entry = super::v3::build_declared(
            &source,
            super::v3::Scope::all(),
            super::v3::DEFAULT_MAX_LOCK_BYTES,
            BTreeMap::new(),
        )
        .expect("entry should build");
        let lock = super::v3::V3Lock::single_declared("private", entry).expect("lock should build");

        let first = super::v3::render(&lock).expect("lock should render");
        let parsed = super::v3::load(&first).expect("lock should load");
        let second = super::v3::render(&parsed).expect("lock should rerender");
        let expanded = super::v3::validate_declared(
            "private",
            parsed.declared("private").expect("entry should exist"),
        )
        .expect("entry should validate");

        assert_eq!(
            first,
            canonical_lines(
                &fs::read_to_string("testdata/lock/v3_private.lock")
                    .expect("golden v3 lockfile should exist")
            )
        );
        assert_eq!(first, second);
        let mut expected = source;
        with_unknown_additional_properties(&mut expected);
        for operation in expected.operations.values_mut() {
            operation.servers = None;
        }
        assert_eq!(expanded, expected);
        for sentinel in crate::lock_size::PRIVACY_SENTINELS {
            assert!(!first.contains(sentinel), "leaked {sentinel}");
        }
    }

    #[test]
    fn v3_enforces_the_exact_contract_byte_limit() {
        let source =
            crate::openapi::load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml"))
                .expect("fixture should load");
        let measured =
            super::v3::measured_contract_bytes_for_test(&source).expect("contract should measure");

        assert!(super::v3::build_declared(
            &source,
            super::v3::Scope::all(),
            measured,
            BTreeMap::new(),
        )
        .is_ok());
        let error = super::v3::build_declared(
            &source,
            super::v3::Scope::all(),
            measured - 1,
            BTreeMap::new(),
        )
        .expect_err("one byte below the payload should fail");
        assert!(error.to_string().contains("exceeds"));
    }

    #[test]
    fn v3_revalidates_contract_bytes_and_digest() {
        let source =
            crate::openapi::load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml"))
                .expect("fixture should load");

        assert!(super::v3::tampered_contract_bytes_error_for_test(&source)
            .expect_err("tampered byte count should fail")
            .to_string()
            .contains("contract byte count mismatch"));
        assert!(super::v3::tampered_contract_digest_error_for_test(&source)
            .expect_err("tampered contract digest should fail")
            .to_string()
            .contains("contract digest mismatch"));
    }

    #[test]
    fn v3_rejects_unknown_semantic_fields() {
        let source =
            crate::openapi::load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml"))
                .expect("fixture should load");
        let entry = super::v3::build_declared(
            &source,
            super::v3::Scope::all(),
            super::v3::DEFAULT_MAX_LOCK_BYTES,
            BTreeMap::new(),
        )
        .expect("entry should build");
        let rendered = super::v3::render(
            &super::v3::V3Lock::single_declared("private", entry).expect("lock should build"),
        )
        .expect("lock should render");
        let tampered = rendered.replace(
            "    source: openapi",
            "    source: openapi\n    unexpected: true",
        );

        let error = super::v3::load(&tampered).expect_err("unknown field should fail");

        assert!(error
            .to_string()
            .contains("failed to parse api.lock v3 YAML"));
    }

    #[test]
    fn v3_rejects_api_names_with_surrounding_whitespace() {
        let rendered = fs::read_to_string("testdata/lock/v3_private.lock")
            .expect("v3 fixture should be readable")
            .replace("  private:", "  ' private ':");

        let error = super::v3::load(&rendered).expect_err("noncanonical api name should fail");

        assert!(error.to_string().contains("surrounding whitespace"));
    }

    #[test]
    fn v3_rejects_unknown_fields_in_nested_observed_shapes() {
        let rendered = "\
version: 3
apis:
  portfolio:
    provenance: observed
    shape:
      kind: object
      observations: 1
      properties:
        value:
          observations: 1
          shape:
            kind: string
            unexpected: true
";

        let error =
            super::v3::load(rendered).expect_err("nested observed fields must be strict in v3");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn v3_rejects_noncanonical_stored_scope_selectors() {
        let source =
            crate::openapi::load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml"))
                .expect("fixture should load");

        let error = super::v3::build_declared(
            &source,
            super::v3::Scope::operations(vec!["get /accounts".to_string()]),
            super::v3::DEFAULT_MAX_LOCK_BYTES,
            BTreeMap::new(),
        )
        .expect_err("stored selectors must use canonical uppercase methods");

        assert!(error.to_string().contains("canonical"));
    }

    #[test]
    fn v3_rejects_scope_contract_mismatches() {
        let source =
            crate::openapi::load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml"))
                .expect("fixture should load");

        let error = super::v3::build_declared(
            &source,
            super::v3::Scope::operations(vec!["GET /users".to_string()]),
            super::v3::DEFAULT_MAX_LOCK_BYTES,
            BTreeMap::new(),
        )
        .expect_err("scope must exactly describe stored operations");

        assert!(error.to_string().contains("does not match"));
    }
}
