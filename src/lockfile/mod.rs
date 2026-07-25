use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::contract::ApiContract;
use crate::observed::{apply_map_annotations, merge as merge_shapes, Shape};

#[doc(hidden)]
pub mod v3;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiLock {
    version: u8,
    #[serde(rename = "apis")]
    legacy_declared: BTreeMap<String, LockedApi>,
    #[serde(skip)]
    declared: BTreeMap<String, v3::DeclaredEntry>,
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
struct LockedOperation {
    method: String,
    path: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct VerifyTarget {
    name: String,
    operations: BTreeSet<LockedOperation>,
    observed_shape: Option<Shape>,
}

impl VerifyTarget {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn observed_shape(&self) -> Option<&Shape> {
        self.observed_shape.as_ref()
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
        .keys()
        .map(|key| LockedOperation {
            method: key.method.as_str().to_string(),
            path: key.path.clone(),
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
        declared: BTreeMap::new(),
        observed: BTreeMap::new(),
    })
}

pub fn render(lock: &ApiLock) -> Result<String> {
    if lock.version == 1 {
        return serde_yaml::to_string(lock).context("failed to serialize lockfile");
    }
    if lock.version == 3 {
        return v3::render(&v3::V3Lock::from_parts(
            lock.declared.clone(),
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
                declared,
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
        declared: BTreeMap::from([(name, entry)]),
        observed: BTreeMap::new(),
    };
    v3::render(&v3::V3Lock::from_parts(
        lock.declared.clone(),
        BTreeMap::new(),
    ))?;
    Ok(lock)
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
            return Err(anyhow!(
                "cannot migrate api.lock to v3; migration requires original sources for: {}",
                remaining.join(", ")
            ));
        }
        lock.legacy_declared.clear();
    }
    lock.version = 3;
    lock.declared.insert(name, entry);
    v3::render(&v3::V3Lock::from_parts(
        lock.declared.clone(),
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
    if lock.legacy_declared.contains_key(name) || lock.declared.contains_key(name) {
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
            declared: BTreeMap::new(),
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
        declared: BTreeMap::new(),
        observed,
    })
}

pub fn select_verify_target(lock: &ApiLock, name: &str) -> Result<VerifyTarget> {
    let name = normalized_name(name)?;
    if let Some(shape) = lock.observed.get(name) {
        return Ok(VerifyTarget {
            name: name.to_string(),
            operations: BTreeSet::new(),
            observed_shape: Some(shape.clone()),
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
        operations,
        observed_shape: None,
    })
}

pub fn compare_verify_target(target: &VerifyTarget, current: &ApiContract) -> Vec<VerifyChange> {
    let current_operations: BTreeSet<_> = current
        .operations
        .keys()
        .map(|key| LockedOperation {
            method: key.method.as_str().to_string(),
            path: key.path.clone(),
        })
        .collect();
    let mut changes = Vec::new();

    for operation in target.operations.difference(&current_operations) {
        changes.push(VerifyChange {
            kind: VerifyChangeKind::Removed,
            method: operation.method.clone(),
            path: operation.path.clone(),
        });
    }

    for operation in current_operations.difference(&target.operations) {
        changes.push(VerifyChange {
            kind: VerifyChangeKind::Added,
            method: operation.method.clone(),
            path: operation.path.clone(),
        });
    }

    changes
}

fn normalized_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("api name cannot be empty"));
    }

    Ok(name)
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    use super::*;

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
        assert_eq!(migrated.declared["users"], entry);
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

        assert_eq!(render(&lock).expect("v3 fixture should render"), expected);
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
            declared: BTreeMap::new(),
            observed: BTreeMap::new(),
        };
        let current =
            crate::openapi::load_contract(Path::new("testdata/openapi/verify_current.yaml"))
                .expect("fixture should load");

        let target = select_verify_target(&lock, "users").expect("target should select");

        assert_eq!(target.name(), "users");
        assert_eq!(
            compare_verify_target(&target, &current),
            vec![
                VerifyChange {
                    kind: VerifyChangeKind::Removed,
                    method: "GET".to_string(),
                    path: "/users".to_string(),
                },
                VerifyChange {
                    kind: VerifyChangeKind::Removed,
                    method: "GET".to_string(),
                    path: "/zeta".to_string(),
                },
                VerifyChange {
                    kind: VerifyChangeKind::Added,
                    method: "POST".to_string(),
                    path: "/users".to_string(),
                },
                VerifyChange {
                    kind: VerifyChangeKind::Added,
                    method: "POST".to_string(),
                    path: "/zeta".to_string(),
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
            declared: BTreeMap::new(),
            observed: BTreeMap::new(),
        };
        let current =
            crate::openapi::load_contract(Path::new("testdata/openapi/lock_ordering.yaml"))
                .expect("fixture should load");

        let target = select_verify_target(&lock, "users").expect("target should select");

        assert_eq!(
            compare_verify_target(&target, &current),
            vec![VerifyChange {
                kind: VerifyChangeKind::Added,
                method: "POST".to_string(),
                path: "/users".to_string(),
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
            declared: BTreeMap::new(),
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
            declared: BTreeMap::new(),
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
    fn load_rejects_an_unsupported_lockfile_version() {
        let error = load(Path::new("testdata/lock/verify_unsupported_version.lock"))
            .expect_err("version 3 lockfile should be rejected");

        assert!(error.to_string().contains("unsupported api.lock version 4"));
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
            declared: BTreeMap::new(),
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
    fn v3_interns_repeated_schemas_and_expands_the_original_contract() {
        use crate::contract::{HttpMethod, Operation, OperationKey, Response, Schema, SchemaKind};

        let schema = Schema {
            kind: SchemaKind::String,
            nullable: false,
            format: Some("uuid".to_string()),
            enum_values: Vec::new(),
            properties: BTreeMap::new(),
        };
        let operation = |path: &str| {
            (
                OperationKey {
                    method: HttpMethod::Get,
                    path: path.to_string(),
                },
                Operation {
                    auth: BTreeMap::new(),
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
    fn v3_builds_and_round_trips_a_deterministic_declared_entry() {
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
            fs::read_to_string("testdata/lock/v3_private.lock")
                .expect("golden v3 lockfile should exist")
        );
        assert_eq!(first, second);
        assert_eq!(expanded, source);
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
}
