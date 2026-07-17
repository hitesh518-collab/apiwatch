use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::contract::ApiContract;
use crate::observed::{merge as merge_shapes, Shape};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiLock {
    version: u8,
    apis: BTreeMap<String, LockedApi>,
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
}

impl VerifyTarget {
    pub fn name(&self) -> &str {
        &self.name
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
        apis,
        observed: BTreeMap::new(),
    })
}

pub fn render(lock: &ApiLock) -> Result<String> {
    if lock.version == 1 {
        return serde_yaml::to_string(lock).context("failed to serialize lockfile");
    }

    let mut apis = BTreeMap::new();
    for (name, api) in &lock.apis {
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
        version => Err(anyhow!("unsupported api.lock version {version}")),
    }
}

pub fn record_observed(
    lock: &mut ApiLock,
    name: &str,
    incoming: Shape,
    merge_existing: bool,
) -> Result<()> {
    let name = normalized_name(name)?;
    if lock.apis.contains_key(name) {
        return Err(anyhow!("api {name} is declared and cannot be recorded as observed"));
    }

    match lock.observed.get_mut(name) {
        Some(existing) if merge_existing => merge_shapes(existing, &incoming),
        Some(_) => return Err(anyhow!("api {name} already exists; use --merge")),
        None if merge_existing => return Err(anyhow!("observed api {name} was not found")),
        None => {
            lock.observed.insert(name.to_string(), incoming);
        }
    }

    lock.version = 2;
    Ok(())
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
        apis,
        observed,
    })
}

pub fn select_verify_target(lock: &ApiLock, name: &str) -> Result<VerifyTarget> {
    let name = normalized_name(name)?;
    let api = lock
        .apis
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
    use std::path::Path;

    use super::*;

    #[test]
    fn compare_verify_target_reports_removed_before_added_in_order() {
        let lock = ApiLock {
            version: 1,
            apis: BTreeMap::from([(
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
            apis: BTreeMap::from([(
                "users".to_string(),
                LockedApi {
                    source: "openapi".to_string(),
                    operations: vec![LockedOperation {
                        method: "get".to_string(),
                        path: "/users".to_string(),
                    }],
                },
            )]),
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
            apis: BTreeMap::from([(
                "users".to_string(),
                LockedApi {
                    source: "openapi".to_string(),
                    operations: vec![LockedOperation {
                        method: "BOGUS".to_string(),
                        path: "/users".to_string(),
                    }],
                },
            )]),
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
            apis: BTreeMap::from([(
                "users".to_string(),
                LockedApi {
                    source: "openapi".to_string(),
                    operations: vec![LockedOperation {
                        method: "GET".to_string(),
                        path: "/users\u{0001}".to_string(),
                    }],
                },
            )]),
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

        assert!(error.to_string().contains("unsupported api.lock version 3"));
    }

    #[test]
    fn recording_into_v1_preserves_declared_operations_and_renders_v2() {
        let mut lock = load(Path::new("testdata/lock/verify_users.lock"))
            .expect("v1 lock should load");
        let shape = crate::observed::infer(&serde_json::json!({
            "id": 1,
            "token": "super-secret-token"
        }));

        record_observed(&mut lock, "portfolio", shape, false)
            .expect("new observed entry should be recorded");
        let rendered = render(&lock).expect("v2 lock should render");

        assert!(rendered.starts_with("version: 2\n"));
        assert!(rendered.contains("provenance: declared"));
        assert!(rendered.contains("provenance: observed"));
        assert!(rendered.contains("path: /users"));
        assert!(!rendered.contains("super-secret-token"));
    }
}
