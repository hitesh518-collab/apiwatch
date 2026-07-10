use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use crate::contract::ApiContract;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ApiLock {
    version: u8,
    apis: BTreeMap<String, LockedApi>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct LockedApi {
    source: String,
    operations: Vec<LockedOperation>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct LockedOperation {
    method: String,
    path: String,
}

pub fn from_contract(name: &str, contract: &ApiContract) -> Result<ApiLock> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("api name cannot be empty"));
    }

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
