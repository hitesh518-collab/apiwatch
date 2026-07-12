use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::contract::ApiContract;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiLock {
    version: u8,
    apis: BTreeMap<String, LockedApi>,
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

    Ok(ApiLock { version: 1, apis })
}

pub fn render(lock: &ApiLock) -> Result<String> {
    serde_yaml::to_string(lock).context("failed to serialize lockfile")
}

pub fn load(path: &Path) -> Result<ApiLock> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read api.lock {}", path.display()))?;
    let lock: ApiLock = serde_yaml::from_str(&contents).context("failed to parse api.lock YAML")?;

    if lock.version != 1 {
        return Err(anyhow!("unsupported api.lock version {}", lock.version));
    }

    Ok(lock)
}

pub fn select_verify_target(lock: &ApiLock, name: &str) -> Result<VerifyTarget> {
    let name = normalized_name(name)?;
    let api = lock
        .apis
        .get(name)
        .ok_or_else(|| anyhow!("api {name} not found in lockfile"))?;

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
            .expect_err("version 2 lockfile should be rejected");

        assert!(error.to_string().contains("unsupported api.lock version 2"));
    }
}
