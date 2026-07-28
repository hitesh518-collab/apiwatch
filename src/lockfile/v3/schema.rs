use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};

use crate::contract::{
    AdditionalProperties, ApiContract, AuthRequirement, HttpMethod, Operation, OperationIdentity,
    OperationKey, Parameter, ParameterKey, ParameterLocation, Property, RequestBody, Response,
    Schema,
};
use crate::openapi::{
    identity::{canonical_media_type, canonical_path_template},
    merge_all_of,
};

use super::{
    canonical, Contract, WireAuth, WireOperation, WireParameter, WireProperty, WireSchema,
};

pub(super) fn intern_contract(contract: &ApiContract) -> Result<Contract> {
    let mut schemas = BTreeMap::new();
    let operations = contract
        .operations
        .iter()
        .map(|(_identity, operation)| {
            let auth = operation
                .auth
                .iter()
                .map(|(name, requirement)| {
                    let mut scopes = requirement.scopes.clone();
                    scopes.sort();
                    scopes.dedup();
                    (
                        name.clone(),
                        WireAuth {
                            kind: requirement.kind,
                            scopes,
                        },
                    )
                })
                .collect();
            let parameters = operation
                .parameters
                .iter()
                .map(|(parameter_key, parameter)| {
                    Ok((
                        format!(
                            "{}:{}",
                            parameter_key.location.as_str(),
                            if parameter_key.location == ParameterLocation::Path {
                                &parameter.name
                            } else {
                                &parameter_key.name
                            }
                        ),
                        WireParameter {
                            required: parameter.required,
                            schema: intern_schema(&parameter.schema, &mut schemas)?,
                        },
                    ))
                })
                .collect::<Result<_>>()?;
            let request_body = operation
                .request_body
                .as_ref()
                .map(|body| intern_content(&body.content, &mut schemas))
                .transpose()?;
            let responses = operation
                .responses
                .iter()
                .map(|(status, response)| {
                    Ok((
                        status.clone(),
                        intern_content(&response.content, &mut schemas)?,
                    ))
                })
                .collect::<Result<_>>()?;
            Ok((
                format!("{} {}", operation.key.method.as_str(), operation.key.path),
                WireOperation {
                    auth,
                    parameters,
                    request_body,
                    responses,
                },
            ))
        })
        .collect::<Result<_>>()?;
    Ok(Contract {
        operations,
        schemas,
    })
}

fn intern_content(
    content: &BTreeMap<String, Schema>,
    schemas: &mut BTreeMap<String, WireSchema>,
) -> Result<BTreeMap<String, String>> {
    content
        .iter()
        .map(|(content_type, schema)| Ok((content_type.clone(), intern_schema(schema, schemas)?)))
        .collect()
}

fn intern_schema(schema: &Schema, schemas: &mut BTreeMap<String, WireSchema>) -> Result<String> {
    let mut properties: BTreeMap<String, WireProperty> = schema
        .properties
        .iter()
        .map(|(name, property)| {
            Ok((
                name.clone(),
                WireProperty {
                    required: property.required,
                    schema: intern_schema(&property.schema, schemas)?,
                },
            ))
        })
        .collect::<Result<_>>()?;
    if matches!(
        schema.kind,
        crate::contract::SchemaKind::OneOf | crate::contract::SchemaKind::AnyOf
    ) {
        let prefix = match schema.kind {
            crate::contract::SchemaKind::OneOf => "oneOf",
            _ => "anyOf",
        };
        for (index, branch) in schema.branches.iter().enumerate() {
            properties.insert(
                format!("{prefix}[{index}]"),
                WireProperty {
                    required: true,
                    schema: intern_schema(branch, schemas)?,
                },
            );
        }
    }
    let mut enum_values = schema.enum_values.clone();
    enum_values.sort();
    enum_values.dedup();
    let wire = WireSchema {
        kind: schema.kind.clone(),
        nullable: schema.nullable,
        format: schema.format.clone(),
        enum_values,
        properties,
    };
    insert_wire_schema(wire, schemas, &canonical::schema_id)
}

fn insert_wire_schema<F>(
    wire: WireSchema,
    schemas: &mut BTreeMap<String, WireSchema>,
    digest: &F,
) -> Result<String>
where
    F: Fn(&WireSchema) -> Result<String>,
{
    let id = digest(&wire)?;
    if let Some(existing) = schemas.get(&id) {
        if existing != &wire {
            return Err(anyhow!("schema digest collision"));
        }
    } else {
        schemas.insert(id.clone(), wire);
    }
    Ok(id)
}

#[cfg(test)]
pub(super) fn forced_collision_error() -> Result<()> {
    let mut schemas = BTreeMap::new();
    let first = WireSchema {
        kind: crate::contract::SchemaKind::String,
        nullable: false,
        format: None,
        enum_values: Vec::new(),
        properties: BTreeMap::new(),
    };
    let second = WireSchema {
        kind: crate::contract::SchemaKind::Integer,
        nullable: false,
        format: None,
        enum_values: Vec::new(),
        properties: BTreeMap::new(),
    };
    let digest = |_: &WireSchema| Ok("sha256:forced".to_string());
    insert_wire_schema(first, &mut schemas, &digest)?;
    insert_wire_schema(second, &mut schemas, &digest)?;
    Ok(())
}

pub(super) fn expand_contract(contract: &Contract) -> Result<ApiContract> {
    validate_schema_table(contract)?;
    let mut operations = BTreeMap::new();
    for (key, operation) in &contract.operations {
        let key = parse_operation_key(key)?;
        let (canonical_path, placeholders) = canonical_path_template(&key.path)?;
        let identity = OperationIdentity {
            method: key.method,
            path: canonical_path,
        };
        let auth = operation
            .auth
            .iter()
            .map(|(name, requirement)| {
                Ok((
                    name.clone(),
                    AuthRequirement {
                        name: name.clone(),
                        kind: requirement.kind,
                        scopes: requirement.scopes.clone(),
                    },
                ))
            })
            .collect::<Result<_>>()?;
        let parameters = operation
            .parameters
            .iter()
            .map(|(key, parameter)| {
                let key = parse_parameter_key(key)?;
                let name = key.name.clone();
                let key = canonicalize_parameter_key(key, &placeholders)?;
                Ok((
                    key.clone(),
                    Parameter {
                        name,
                        required: parameter.required,
                        schema: expand_schema(&parameter.schema, &contract.schemas)?,
                    },
                ))
            })
            .collect::<Result<_>>()?;
        let request_body = operation
            .request_body
            .as_ref()
            .map(|content| -> Result<RequestBody> {
                Ok(RequestBody {
                    required: None,
                    content: expand_content(content, &contract.schemas)?,
                })
            })
            .transpose()?;
        let responses = operation
            .responses
            .iter()
            .map(|(status, content)| {
                Ok((
                    status.clone(),
                    Response {
                        content: expand_content(content, &contract.schemas)?,
                    },
                ))
            })
            .collect::<Result<_>>()?;
        let operation = Operation {
            key,
            auth,
            servers: None,
            parameters,
            request_body,
            responses,
        };
        if operations.insert(identity.clone(), operation).is_some() {
            return Err(anyhow!(
                "ambiguous operation identity {} {}",
                identity.method.as_str(),
                identity.path
            ));
        }
    }
    Ok(ApiContract { operations })
}

fn canonicalize_parameter_key(key: ParameterKey, placeholders: &[String]) -> Result<ParameterKey> {
    if key.location != ParameterLocation::Path {
        return Ok(key);
    }
    let slot = placeholders
        .iter()
        .position(|name| name == &key.name)
        .ok_or_else(|| {
            anyhow!(
                "path parameter {} is not bound to a path template placeholder",
                key.name
            )
        })?;
    Ok(ParameterKey {
        location: ParameterLocation::Path,
        name: format!("{{{slot}}}"),
    })
}

fn expand_content(
    content: &BTreeMap<String, String>,
    schemas: &BTreeMap<String, WireSchema>,
) -> Result<BTreeMap<String, Schema>> {
    content
        .iter()
        .map(|(content_type, id)| {
            Ok((
                canonical_media_type(content_type)?,
                expand_schema(id, schemas)?,
            ))
        })
        .collect()
}

fn expand_schema(id: &str, schemas: &BTreeMap<String, WireSchema>) -> Result<Schema> {
    let wire = schemas
        .get(id)
        .ok_or_else(|| anyhow!("missing schema reference"))?;
    let mut properties: BTreeMap<String, crate::contract::Property> = wire
        .properties
        .iter()
        .map(|(name, property)| {
            Ok((
                name.clone(),
                Property {
                    required: property.required,
                    schema: Box::new(expand_schema(&property.schema, schemas)?),
                },
            ))
        })
        .collect::<Result<_>>()?;
    let branches = match wire.kind {
        crate::contract::SchemaKind::OneOf | crate::contract::SchemaKind::AnyOf => {
            let prefix = if matches!(wire.kind, crate::contract::SchemaKind::OneOf) {
                "oneOf"
            } else {
                "anyOf"
            };
            let mut legacy = properties
                .iter()
                .filter_map(|(name, property)| {
                    parse_legacy_branch(name, prefix).map(|index| (index, *property.schema.clone()))
                })
                .collect::<Vec<_>>();
            legacy.sort_by_key(|(index, _)| *index);
            for (index, _) in &legacy {
                properties.remove(&format!("{prefix}[{index}]"));
            }
            let mut branches = legacy
                .into_iter()
                .map(|(_, branch)| Ok(branch))
                .collect::<Result<Vec<_>>>()?;
            branches.sort_by_key(Schema::structural_key);
            branches.dedup_by(|left, right| left.structural_key() == right.structural_key());
            branches
        }
        crate::contract::SchemaKind::AllOf => {
            let mut legacy = properties
                .iter()
                .filter_map(|(name, property)| {
                    parse_legacy_branch(name, "allOf")
                        .map(|index| (index, *property.schema.clone()))
                })
                .collect::<Vec<_>>();
            if legacy.is_empty() {
                Vec::new()
            } else {
                legacy.sort_by_key(|(index, _)| *index);
                for (index, _) in &legacy {
                    properties.remove(&format!("allOf[{index}]"));
                }
                let mut branches = legacy
                    .into_iter()
                    .map(|(_, branch)| branch)
                    .collect::<Vec<_>>();
                let mut merged = merge_all_of(std::mem::take(&mut branches))?;
                merged.nullable &= wire.nullable;
                properties.extend(merged.properties);
                return Ok(Schema {
                    kind: merged.kind,
                    nullable: merged.nullable,
                    format: merged.format,
                    enum_values: merged.enum_values,
                    properties,
                    additional_properties: merged.additional_properties,
                    branches: Vec::new(),
                });
            }
        }
        _ => Vec::new(),
    };
    Ok(Schema {
        kind: wire.kind.clone(),
        nullable: wire.nullable,
        format: wire.format.clone(),
        enum_values: wire.enum_values.clone(),
        properties,
        additional_properties: AdditionalProperties::Unknown,
        branches,
    })
}

fn parse_legacy_branch(name: &str, prefix: &str) -> Option<usize> {
    name.strip_prefix(prefix)?
        .strip_prefix('[')?
        .strip_suffix(']')?
        .parse()
        .ok()
}

pub(super) fn validate_schema_table(contract: &Contract) -> Result<()> {
    let roots = schema_roots(contract);
    let mut reachable = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    for root in roots {
        visit_schema(&root, &contract.schemas, &mut visiting, &mut reachable)?;
    }
    if let Some(orphan) = contract.schemas.keys().find(|id| !reachable.contains(*id)) {
        return Err(anyhow!("orphan schema {}", sanitized(orphan)));
    }
    Ok(())
}

fn schema_roots(contract: &Contract) -> Vec<String> {
    let mut roots = Vec::new();
    for operation in contract.operations.values() {
        roots.extend(
            operation
                .parameters
                .values()
                .map(|parameter| parameter.schema.clone()),
        );
        if let Some(content) = &operation.request_body {
            roots.extend(content.values().cloned());
        }
        for content in operation.responses.values() {
            roots.extend(content.values().cloned());
        }
    }
    roots
}

fn visit_schema(
    id: &str,
    schemas: &BTreeMap<String, WireSchema>,
    visiting: &mut BTreeSet<String>,
    reachable: &mut BTreeSet<String>,
) -> Result<()> {
    canonical::validate_digest(id)?;
    if reachable.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.to_string()) {
        return Err(anyhow!("cyclic v3 schema reference"));
    }
    let schema = schemas
        .get(id)
        .ok_or_else(|| anyhow!("missing schema reference {}", sanitized(id)))?;
    let actual = canonical::schema_id(schema)?;
    if actual != id {
        return Err(anyhow!("schema digest mismatch {}", sanitized(id)));
    }
    for property in schema.properties.values() {
        visit_schema(&property.schema, schemas, visiting, reachable)?;
    }
    visiting.remove(id);
    reachable.insert(id.to_string());
    Ok(())
}

pub(super) fn parse_operation_key(value: &str) -> Result<OperationKey> {
    let (method, path) = value
        .split_once(' ')
        .ok_or_else(|| anyhow!("invalid v3 operation key"))?;
    if path.is_empty()
        || !path.starts_with('/')
        || method.is_empty()
        || path.chars().any(char::is_control)
        || method.chars().any(char::is_control)
        || path.chars().any(char::is_whitespace)
    {
        return Err(anyhow!("invalid v3 operation key"));
    }
    let method = match method {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "OPTIONS" => HttpMethod::Options,
        "HEAD" => HttpMethod::Head,
        "TRACE" => HttpMethod::Trace,
        _ => return Err(anyhow!("unsupported v3 operation method")),
    };
    Ok(OperationKey {
        method,
        path: path.to_string(),
    })
}

pub(super) fn parse_parameter_key(value: &str) -> Result<ParameterKey> {
    let (location, name) = value
        .split_once(':')
        .ok_or_else(|| anyhow!("invalid v3 parameter key"))?;
    if name.is_empty() || name.chars().any(char::is_control) {
        return Err(anyhow!("invalid v3 parameter key"));
    }
    let location = match location {
        "path" => ParameterLocation::Path,
        "query" => ParameterLocation::Query,
        "header" => ParameterLocation::Header,
        "cookie" => ParameterLocation::Cookie,
        _ => return Err(anyhow!("unsupported v3 parameter location")),
    };
    Ok(ParameterKey {
        location,
        name: name.to_string(),
    })
}

fn sanitized(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{expand_contract, Contract, WireOperation};

    #[test]
    fn rejects_operations_that_collide_after_path_template_canonicalization() {
        let contract = Contract {
            operations: BTreeMap::from([
                (
                    "GET /users/{id}".to_string(),
                    WireOperation {
                        auth: BTreeMap::new(),
                        parameters: BTreeMap::new(),
                        request_body: None,
                        responses: BTreeMap::new(),
                    },
                ),
                (
                    "GET /users/{name}".to_string(),
                    WireOperation {
                        auth: BTreeMap::new(),
                        parameters: BTreeMap::new(),
                        request_body: None,
                        responses: BTreeMap::new(),
                    },
                ),
            ]),
            schemas: BTreeMap::new(),
        };

        let error = expand_contract(&contract)
            .expect_err("canonical operation identities must not overwrite each other");
        assert!(error.to_string().contains("ambiguous operation identity"));
    }
}
