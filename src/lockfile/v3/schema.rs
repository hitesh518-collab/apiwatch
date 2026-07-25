use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};

use crate::contract::{
    ApiContract, AuthRequirement, HttpMethod, Operation, OperationKey, Parameter, ParameterKey,
    ParameterLocation, Property, RequestBody, Response, Schema,
};

use super::{
    canonical, Contract, WireAuth, WireOperation, WireParameter, WireProperty, WireSchema,
};

pub(super) fn intern_contract(contract: &ApiContract) -> Result<Contract> {
    let mut schemas = BTreeMap::new();
    let operations = contract
        .operations
        .iter()
        .map(|(key, operation)| {
            let auth = operation
                .auth
                .iter()
                .map(|(name, requirement)| {
                    (
                        name.clone(),
                        WireAuth {
                            kind: requirement.kind,
                            scopes: requirement.scopes.clone(),
                        },
                    )
                })
                .collect();
            let parameters = operation
                .parameters
                .iter()
                .map(|(parameter_key, parameter)| {
                    Ok((
                        format!("{}:{}", parameter_key.location.as_str(), parameter_key.name),
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
                format!("{} {}", key.method.as_str(), key.path),
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
    let properties = schema
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
    let wire = WireSchema {
        kind: schema.kind.clone(),
        nullable: schema.nullable,
        format: schema.format.clone(),
        enum_values: schema.enum_values.clone(),
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
    let operations = contract
        .operations
        .iter()
        .map(|(key, operation)| {
            let key = parse_operation_key(key)?;
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
                    Ok((
                        key.clone(),
                        Parameter {
                            name: key.name,
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
            Ok((
                key,
                Operation {
                    auth,
                    parameters,
                    request_body,
                    responses,
                },
            ))
        })
        .collect::<Result<_>>()?;
    Ok(ApiContract { operations })
}

fn expand_content(
    content: &BTreeMap<String, String>,
    schemas: &BTreeMap<String, WireSchema>,
) -> Result<BTreeMap<String, Schema>> {
    content
        .iter()
        .map(|(content_type, id)| Ok((content_type.clone(), expand_schema(id, schemas)?)))
        .collect()
}

fn expand_schema(id: &str, schemas: &BTreeMap<String, WireSchema>) -> Result<Schema> {
    let wire = schemas
        .get(id)
        .ok_or_else(|| anyhow!("missing schema reference"))?;
    let properties = wire
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
    Ok(Schema {
        kind: wire.kind.clone(),
        nullable: wire.nullable,
        format: wire.format.clone(),
        enum_values: wire.enum_values.clone(),
        properties,
    })
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

fn parse_operation_key(value: &str) -> Result<OperationKey> {
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

fn parse_parameter_key(value: &str) -> Result<ParameterKey> {
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
