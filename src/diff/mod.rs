use crate::contract::{
    ApiContract, OperationKey, Parameter, ParameterKey, RequestBody, Response, Schema, SchemaKind,
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

    changes
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
    for (status, old_response) in old {
        let Some(new_response) = new.get(status) else {
            continue;
        };

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

fn diff_request_bodies(
    changes: &mut Vec<Change>,
    operation: &OperationKey,
    old: Option<&RequestBody>,
    new: Option<&RequestBody>,
) {
    let (Some(old), Some(new)) = (old, new) else {
        return;
    };

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
    if usage != SchemaUsage::Request || old_required == new_required {
        return;
    }

    changes.push(Change {
        severity: if new_required {
            Severity::Breaking
        } else {
            Severity::NonBreaking
        },
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
    } else {
        format!("field {path}")
    }
}

fn schema_kind_name(kind: &SchemaKind) -> &'static str {
    match kind {
        SchemaKind::Object => "object",
        SchemaKind::Array => "array",
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
