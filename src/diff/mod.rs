use crate::contract::{ApiContract, OperationKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Breaking,
    Warning,
    NonBreaking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub severity: Severity,
    pub operation: OperationKey,
    pub message: String,
}

pub fn diff_contracts(old: &ApiContract, new: &ApiContract) -> Vec<Change> {
    let mut changes = Vec::new();

    for key in old.operations.keys() {
        if !new.operations.contains_key(key) {
            changes.push(Change {
                severity: Severity::Breaking,
                operation: key.clone(),
                message: "endpoint removed".to_string(),
            });
        }
    }

    for key in new.operations.keys() {
        if !old.operations.contains_key(key) {
            changes.push(Change {
                severity: Severity::NonBreaking,
                operation: key.clone(),
                message: "endpoint added".to_string(),
            });
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{diff_contracts, Severity};
    use crate::openapi::load_contract;

    #[test]
    fn detects_removed_endpoint_as_breaking() {
        let old = load_contract(Path::new("testdata/openapi/endpoint_removed_old.yaml"))
            .expect("old fixture should parse");
        let new = load_contract(Path::new("testdata/openapi/endpoint_removed_new.yaml"))
            .expect("new fixture should parse");

        let changes = diff_contracts(&old, &new);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].severity, Severity::Breaking);
        assert_eq!(changes[0].operation.method.as_str(), "GET");
        assert_eq!(changes[0].operation.path, "/users");
        assert_eq!(changes[0].message, "endpoint removed");
    }

    #[test]
    fn detects_added_endpoint_as_non_breaking() {
        let old = load_contract(Path::new("testdata/openapi/no_breaking_old.yaml"))
            .expect("old fixture should parse");
        let new = load_contract(Path::new("testdata/openapi/no_breaking_new.yaml"))
            .expect("new fixture should parse");

        let changes = diff_contracts(&old, &new);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].severity, Severity::NonBreaking);
        assert_eq!(changes[0].operation.method.as_str(), "GET");
        assert_eq!(changes[0].operation.path, "/teams");
        assert_eq!(changes[0].message, "endpoint added");
    }
}
