use crate::contract::{
    AdditionalProperties, ApiContract, AuthRequirement, OperationKey, Parameter, ParameterKey,
    RequestBody, Response, Schema, SchemaKind,
};

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
    let mut server_changes = Vec::new();

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

    for (key, old_operation) in &old.operations {
        if let Some(new_operation) = new.operations.get(key) {
            diff_auth_requirements(&mut changes, key, &old_operation.auth, &new_operation.auth);
            diff_servers(
                &mut server_changes,
                key,
                old_operation.servers.as_ref(),
                new_operation.servers.as_ref(),
            );
            diff_parameters(
                &mut changes,
                key,
                &old_operation.parameters,
                &new_operation.parameters,
            );
            diff_responses(
                &mut changes,
                key,
                &old_operation.responses,
                &new_operation.responses,
            );
            diff_request_bodies(
                &mut changes,
                key,
                old_operation.request_body.as_ref(),
                new_operation.request_body.as_ref(),
            );
        }
    }

    server_changes.sort_by(|left, right| {
        let severity = |change: &Change| match change.severity {
            Severity::Breaking => 0,
            Severity::Warning => 1,
            Severity::NonBreaking => 2,
        };
        severity(left)
            .cmp(&severity(right))
            .then_with(|| left.operation.cmp(&right.operation))
            .then_with(|| left.message.cmp(&right.message))
    });
    changes.extend(server_changes);
    changes
}

fn diff_servers(
    changes: &mut Vec<Change>,
    operation: &OperationKey,
    old: Option<&std::collections::BTreeSet<crate::contract::ServerTemplate>>,
    new: Option<&std::collections::BTreeSet<crate::contract::ServerTemplate>>,
) {
    let (Some(old), Some(new)) = (old, new) else {
        return;
    };
    for server in old.difference(new) {
        changes.push(Change {
            severity: Severity::Breaking,
            operation: operation.clone(),
            message: format!("server {} removed", server.0),
        });
    }
    for server in new.difference(old) {
        changes.push(Change {
            severity: Severity::NonBreaking,
            operation: operation.clone(),
            message: format!("server {} added", server.0),
        });
    }
}

fn diff_auth_requirements(
    changes: &mut Vec<Change>,
    operation: &OperationKey,
    old: &std::collections::BTreeMap<String, AuthRequirement>,
    new: &std::collections::BTreeMap<String, AuthRequirement>,
) {
    for (name, old_requirement) in old {
        let Some(new_requirement) = new.get(name) else {
            changes.push(Change {
                severity: Severity::NonBreaking,
                operation: operation.clone(),
                message: format!(
                    "authentication {} ({}) removed",
                    old_requirement.name,
                    old_requirement.kind.as_str()
                ),
            });
            continue;
        };

        if old_requirement.kind != new_requirement.kind {
            changes.push(Change {
                severity: Severity::Breaking,
                operation: operation.clone(),
                message: format!(
                    "authentication {} changed from {} to {}",
                    new_requirement.name,
                    old_requirement.kind.as_str(),
                    new_requirement.kind.as_str()
                ),
            });
        }

        diff_auth_scopes(changes, operation, old_requirement, new_requirement);
    }

    for (name, new_requirement) in new {
        if !old.contains_key(name) {
            changes.push(Change {
                severity: Severity::Breaking,
                operation: operation.clone(),
                message: format!(
                    "authentication {} ({}) added",
                    new_requirement.name,
                    new_requirement.kind.as_str()
                ),
            });
        }
    }
}

fn diff_auth_scopes(
    changes: &mut Vec<Change>,
    operation: &OperationKey,
    old: &AuthRequirement,
    new: &AuthRequirement,
) {
    for scope in &new.scopes {
        if !old.scopes.contains(scope) {
            changes.push(Change {
                severity: Severity::Breaking,
                operation: operation.clone(),
                message: format!("authentication {} scope {scope} added", new.name),
            });
        }
    }

    for scope in &old.scopes {
        if !new.scopes.contains(scope) {
            changes.push(Change {
                severity: Severity::NonBreaking,
                operation: operation.clone(),
                message: format!("authentication {} scope {scope} removed", old.name),
            });
        }
    }
}

fn diff_parameters(
    changes: &mut Vec<Change>,
    operation: &OperationKey,
    old: &std::collections::BTreeMap<ParameterKey, Parameter>,
    new: &std::collections::BTreeMap<ParameterKey, Parameter>,
) {
    for (key, old_parameter) in old {
        let Some(new_parameter) = new.get(key) else {
            changes.push(Change {
                severity: Severity::Breaking,
                operation: operation.clone(),
                message: format!(
                    "{} parameter {} removed",
                    key.location.as_str(),
                    old_parameter.name
                ),
            });
            continue;
        };

        diff_parameter_requiredness(changes, operation, key, old_parameter, new_parameter);

        let context = parameter_context(key, new_parameter);
        diff_schema(
            changes,
            operation,
            SchemaUsage::Request,
            &context,
            "",
            &old_parameter.schema,
            &new_parameter.schema,
        );
    }

    for (key, new_parameter) in new {
        if !old.contains_key(key) {
            changes.push(Change {
                severity: if new_parameter.required {
                    Severity::Breaking
                } else {
                    Severity::NonBreaking
                },
                operation: operation.clone(),
                message: format!(
                    "{} parameter {} added as {}",
                    key.location.as_str(),
                    new_parameter.name,
                    required_name(new_parameter.required)
                ),
            });
        }
    }
}

fn diff_parameter_requiredness(
    changes: &mut Vec<Change>,
    operation: &OperationKey,
    key: &ParameterKey,
    old: &Parameter,
    new: &Parameter,
) {
    if old.required == new.required {
        return;
    }

    changes.push(Change {
        severity: if new.required {
            Severity::Breaking
        } else {
            Severity::NonBreaking
        },
        operation: operation.clone(),
        message: format!(
            "{} parameter {} changed from {} to {}",
            key.location.as_str(),
            new.name,
            required_name(old.required),
            required_name(new.required)
        ),
    });
}

fn parameter_context(key: &ParameterKey, parameter: &Parameter) -> String {
    format!("{} parameter {}", key.location.as_str(), parameter.name)
}

fn diff_responses(
    changes: &mut Vec<Change>,
    operation: &OperationKey,
    old: &std::collections::BTreeMap<String, Response>,
    new: &std::collections::BTreeMap<String, Response>,
) {
    for status in old.keys() {
        if !new.contains_key(status) {
            changes.push(Change {
                severity: if is_success_status(status) {
                    Severity::Breaking
                } else {
                    Severity::NonBreaking
                },
                operation: operation.clone(),
                message: format!("response status {status} removed"),
            });
        }
    }

    for status in new.keys() {
        if !old.contains_key(status) {
            changes.push(Change {
                severity: if is_error_status(status) {
                    Severity::Warning
                } else {
                    Severity::NonBreaking
                },
                operation: operation.clone(),
                message: format!("response status {status} added"),
            });
        }
    }

    for (status, old_response) in old {
        let Some(new_response) = new.get(status) else {
            continue;
        };

        for content_type in old_response.content.keys() {
            if !new_response.content.contains_key(content_type) {
                changes.push(Change {
                    severity: Severity::Breaking,
                    operation: operation.clone(),
                    message: format!("response {status} content type {content_type} removed"),
                });
            }
        }

        for content_type in new_response.content.keys() {
            if !old_response.content.contains_key(content_type) {
                changes.push(Change {
                    severity: Severity::Breaking,
                    operation: operation.clone(),
                    message: format!("response {status} content type {content_type} added"),
                });
            }
        }

        for (content_type, old_schema) in &old_response.content {
            let Some(new_schema) = new_response.content.get(content_type) else {
                continue;
            };

            let context = format!("response {status} {content_type}");
            diff_schema(
                changes,
                operation,
                SchemaUsage::Response,
                &context,
                "",
                old_schema,
                new_schema,
            );
        }
    }
}

fn is_success_status(status: &str) -> bool {
    status.starts_with('2')
}

fn is_error_status(status: &str) -> bool {
    status.starts_with('4') || status.starts_with('5') || status == "default"
}

pub fn diff_request_bodies(
    changes: &mut Vec<Change>,
    operation: &OperationKey,
    old: Option<&RequestBody>,
    new: Option<&RequestBody>,
) {
    let (old, new) = match (old, new) {
        (None, Some(new)) => {
            let (severity, requiredness) = if new.required == Some(true) {
                (Severity::Breaking, "required")
            } else {
                (Severity::NonBreaking, "optional")
            };
            changes.push(Change {
                severity,
                operation: operation.clone(),
                message: format!("request body added as {requiredness}"),
            });
            return;
        }
        (Some(_), None) => {
            changes.push(Change {
                severity: Severity::Breaking,
                operation: operation.clone(),
                message: "request body removed".to_string(),
            });
            return;
        }
        (None, None) => return,
        (Some(old), Some(new)) => (old, new),
    };

    if let (Some(old_required), Some(new_required)) = (old.required, new.required) {
        if old_required != new_required {
            let (severity, old_requiredness, new_requiredness) = if new_required {
                (Severity::Breaking, "optional", "required")
            } else {
                (Severity::NonBreaking, "required", "optional")
            };
            changes.push(Change {
                severity,
                operation: operation.clone(),
                message: format!(
                    "request body changed from {old_requiredness} to {new_requiredness}"
                ),
            });
        }
    }

    for content_type in old.content.keys() {
        if !new.content.contains_key(content_type) {
            changes.push(Change {
                severity: Severity::Breaking,
                operation: operation.clone(),
                message: format!("request content type {content_type} removed"),
            });
        }
    }

    for content_type in new.content.keys() {
        if !old.content.contains_key(content_type) {
            changes.push(Change {
                severity: Severity::NonBreaking,
                operation: operation.clone(),
                message: format!("request content type {content_type} added"),
            });
        }
    }

    for (content_type, old_schema) in &old.content {
        let Some(new_schema) = new.content.get(content_type) else {
            continue;
        };

        let context = format!("request {content_type}");
        diff_schema(
            changes,
            operation,
            SchemaUsage::Request,
            &context,
            "",
            old_schema,
            new_schema,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SchemaUsage {
    Request,
    Response,
}

fn diff_schema(
    changes: &mut Vec<Change>,
    operation: &OperationKey,
    usage: SchemaUsage,
    context: &str,
    path: &str,
    old: &Schema,
    new: &Schema,
) {
    if old.kind != new.kind {
        changes.push(Change {
            severity: Severity::Breaking,
            operation: operation.clone(),
            message: format!(
                "{context} {} type changed from {} to {}",
                schema_target(path),
                schema_kind_name(&old.kind),
                schema_kind_name(&new.kind)
            ),
        });
    }

    if old.nullable != new.nullable {
        changes.push(Change {
            severity: nullable_change_severity(usage, old.nullable, new.nullable),
            operation: operation.clone(),
            message: format!(
                "{context} {} nullable changed from {} to {}",
                schema_target(path),
                old.nullable,
                new.nullable
            ),
        });
    }

    if old.format != new.format {
        changes.push(Change {
            severity: Severity::Warning,
            operation: operation.clone(),
            message: format!(
                "{context} {} format changed from {} to {}",
                schema_target(path),
                format_name(old.format.as_deref()),
                format_name(new.format.as_deref())
            ),
        });
    }

    for value in &new.enum_values {
        if !old.enum_values.contains(value) {
            changes.push(Change {
                severity: enum_value_added_severity(usage),
                operation: operation.clone(),
                message: format!("{context} {} enum value {value} added", schema_target(path)),
            });
        }
    }

    for value in &old.enum_values {
        if !new.enum_values.contains(value) {
            changes.push(Change {
                severity: enum_value_removed_severity(usage),
                operation: operation.clone(),
                message: format!(
                    "{context} {} enum value {value} removed",
                    schema_target(path)
                ),
            });
        }
    }

    if matches!(old.kind, SchemaKind::OneOf | SchemaKind::AnyOf) && old.kind == new.kind {
        diff_branches(changes, operation, usage, context, path, old, new);
        return;
    }

    for name in old.properties.keys() {
        if !new.properties.contains_key(name) {
            changes.push(Change {
                severity: field_removed_severity(usage),
                operation: operation.clone(),
                message: format!("{context} field {} removed", field_path(path, name)),
            });
        }
    }

    for (name, new_property) in &new.properties {
        if !old.properties.contains_key(name) {
            changes.push(Change {
                severity: field_added_severity(usage, new_property.required),
                operation: operation.clone(),
                message: field_added_message(context, path, name, usage, new_property.required),
            });
        }
    }

    for (name, old_property) in &old.properties {
        let Some(new_property) = new.properties.get(name) else {
            continue;
        };

        let path = field_path(path, name);
        diff_requiredness(
            changes,
            operation,
            usage,
            context,
            &path,
            old_property.required,
            new_property.required,
        );
        diff_schema(
            changes,
            operation,
            usage,
            context,
            &path,
            &old_property.schema,
            &new_property.schema,
        );
    }

    diff_additional_properties(changes, operation, usage, context, path, old, new);
}

fn diff_branches(
    changes: &mut Vec<Change>,
    operation: &OperationKey,
    usage: SchemaUsage,
    context: &str,
    path: &str,
    old: &Schema,
    new: &Schema,
) {
    let mut old_remaining: Vec<_> = old.branches.iter().enumerate().collect();
    let mut new_remaining: Vec<_> = new.branches.iter().enumerate().collect();
    old_remaining.retain(|(_, old_branch)| {
        if let Some(index) = new_remaining
            .iter()
            .position(|(_, new_branch)| old_branch.structural_key() == new_branch.structural_key())
        {
            new_remaining.remove(index);
            false
        } else {
            true
        }
    });
    let mut pairs = Vec::new();
    for old_position in (0..old_remaining.len()).rev() {
        let (_, old_branch) = old_remaining[old_position];
        let shape = old_branch.shape_key();
        let old_count = old_remaining
            .iter()
            .filter(|(_, branch)| branch.shape_key() == shape)
            .count();
        let matching: Vec<_> = new_remaining
            .iter()
            .enumerate()
            .filter(|(_, (_, branch))| branch.shape_key() == shape)
            .map(|(index, _)| index)
            .collect();
        if old_count == 1 && matching.len() == 1 {
            let new_branch = new_remaining.remove(matching[0]);
            pairs.push((old_remaining.remove(old_position), new_branch));
        }
    }
    let name = schema_kind_name(&new.kind);
    for ((_, old_branch), (new_index, new_branch)) in pairs {
        let branch_path = field_path(path, &format!("{name}[{new_index}]"));
        diff_schema(
            changes,
            operation,
            usage,
            context,
            &branch_path,
            old_branch,
            new_branch,
        );
    }
    for (old_index, _) in old_remaining {
        changes.push(Change {
            severity: branch_removed_severity(usage),
            operation: operation.clone(),
            message: format!(
                "{context} field {} removed",
                field_path(
                    path,
                    &format!("{}[{old_index}]", schema_kind_name(&old.kind))
                )
            ),
        });
    }
    for (new_index, _) in new_remaining {
        changes.push(Change {
            severity: branch_added_severity(usage),
            operation: operation.clone(),
            message: format!(
                "{context} field {} added",
                field_path(path, &format!("{name}[{new_index}]"))
            ),
        });
    }
}

fn branch_removed_severity(usage: SchemaUsage) -> Severity {
    match usage {
        SchemaUsage::Request => Severity::Breaking,
        SchemaUsage::Response => Severity::NonBreaking,
    }
}

fn branch_added_severity(usage: SchemaUsage) -> Severity {
    match usage {
        SchemaUsage::Request => Severity::NonBreaking,
        SchemaUsage::Response => Severity::Breaking,
    }
}

fn diff_additional_properties(
    changes: &mut Vec<Change>,
    operation: &OperationKey,
    usage: SchemaUsage,
    context: &str,
    path: &str,
    old: &Schema,
    new: &Schema,
) {
    use AdditionalProperties::{Any, Forbidden, Schema, Unknown};

    if matches!(old.additional_properties, Unknown) || matches!(new.additional_properties, Unknown)
    {
        return;
    }

    match (&old.additional_properties, &new.additional_properties) {
        (Schema(old_schema), Schema(new_schema)) => {
            let path = field_path(path, "additionalProperties");
            diff_schema(
                changes, operation, usage, context, &path, old_schema, new_schema,
            );
        }
        (old_policy, new_policy) if old_policy == new_policy => {}
        (old_policy, new_policy) => {
            let path = field_path(path, "additionalProperties");
            let narrowing = matches!(
                (old_policy, new_policy),
                (Any, Forbidden) | (Any, Schema(_)) | (Schema(_), Forbidden)
            );
            let severity = match (usage, narrowing) {
                (SchemaUsage::Request, true) | (SchemaUsage::Response, false) => Severity::Breaking,
                (SchemaUsage::Request, false) | (SchemaUsage::Response, true) => {
                    Severity::NonBreaking
                }
            };
            changes.push(Change {
                severity,
                operation: operation.clone(),
                message: format!(
                    "{context} {} changed from {} to {}",
                    schema_target(&path),
                    additional_properties_name(old_policy),
                    additional_properties_name(new_policy)
                ),
            });
        }
    }
}

fn additional_properties_name(policy: &AdditionalProperties) -> &'static str {
    match policy {
        AdditionalProperties::Unknown => "unknown",
        AdditionalProperties::Forbidden => "forbidden",
        AdditionalProperties::Any => "any",
        AdditionalProperties::Schema(_) => "schema",
    }
}

fn diff_requiredness(
    changes: &mut Vec<Change>,
    operation: &OperationKey,
    usage: SchemaUsage,
    context: &str,
    path: &str,
    old_required: bool,
    new_required: bool,
) {
    let severity = match (usage, old_required, new_required) {
        (SchemaUsage::Request, false, true) => Severity::Breaking,
        (SchemaUsage::Request, true, false) => Severity::NonBreaking,
        (SchemaUsage::Response, true, false) => Severity::Breaking,
        (SchemaUsage::Response, false, true) => Severity::NonBreaking,
        (_, _, _) => return,
    };

    changes.push(Change {
        severity,
        operation: operation.clone(),
        message: format!(
            "{context} field {path} changed from {} to {}",
            required_name(old_required),
            required_name(new_required)
        ),
    });
}

fn nullable_change_severity(
    usage: SchemaUsage,
    old_nullable: bool,
    new_nullable: bool,
) -> Severity {
    match usage {
        SchemaUsage::Request => {
            if old_nullable && !new_nullable {
                Severity::Breaking
            } else {
                Severity::NonBreaking
            }
        }
        SchemaUsage::Response => {
            if !old_nullable && new_nullable {
                Severity::Breaking
            } else {
                Severity::NonBreaking
            }
        }
    }
}

fn enum_value_added_severity(usage: SchemaUsage) -> Severity {
    match usage {
        SchemaUsage::Request => Severity::NonBreaking,
        SchemaUsage::Response => Severity::Breaking,
    }
}

fn enum_value_removed_severity(usage: SchemaUsage) -> Severity {
    match usage {
        SchemaUsage::Request => Severity::Breaking,
        SchemaUsage::Response => Severity::NonBreaking,
    }
}

fn field_removed_severity(_usage: SchemaUsage) -> Severity {
    Severity::Breaking
}

fn field_added_severity(usage: SchemaUsage, required: bool) -> Severity {
    match usage {
        SchemaUsage::Request => {
            if required {
                Severity::Breaking
            } else {
                Severity::NonBreaking
            }
        }
        SchemaUsage::Response => Severity::NonBreaking,
    }
}

fn field_added_message(
    context: &str,
    path: &str,
    name: &str,
    usage: SchemaUsage,
    required: bool,
) -> String {
    let path = field_path(path, name);
    match usage {
        SchemaUsage::Request => format!(
            "{context} field {path} added as {}",
            required_name(required)
        ),
        SchemaUsage::Response => format!("{context} field {path} added"),
    }
}

fn field_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}.{name}")
    }
}

fn required_name(required: bool) -> &'static str {
    if required {
        "required"
    } else {
        "optional"
    }
}

fn schema_target(path: &str) -> String {
    if path.is_empty() {
        "schema".to_string()
    } else if path == "additionalProperties" {
        path.to_string()
    } else {
        format!("field {path}")
    }
}

fn format_name(format: Option<&str>) -> &str {
    format.unwrap_or("none")
}

fn schema_kind_name(kind: &SchemaKind) -> &'static str {
    match kind {
        SchemaKind::Object => "object",
        SchemaKind::Array => "array",
        SchemaKind::OneOf => "oneOf",
        SchemaKind::AllOf => "allOf",
        SchemaKind::AnyOf => "anyOf",
        SchemaKind::String => "string",
        SchemaKind::Integer => "integer",
        SchemaKind::Number => "number",
        SchemaKind::Boolean => "boolean",
        SchemaKind::Unknown => "unknown",
    }
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
