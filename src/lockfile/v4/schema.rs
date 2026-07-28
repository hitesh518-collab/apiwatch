use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};

use super::{
    canonical, Contract, WireAdditionalProperties, WireAuth, WireOperation, WireParameter,
    WireProperty, WireRequestBody, WireSchema,
};
use crate::contract::{
    AdditionalProperties, ApiContract, AuthRequirement, HttpMethod, Operation, OperationKey,
    Parameter, ParameterKey, ParameterLocation, Property, RequestBody, Response, Schema,
};
use crate::openapi::identity::canonical_media_type;

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
                .map(|(key, parameter)| {
                    Ok((
                        format!("{}:{}", key.location.as_str(), key.name),
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
                .map(|body| -> Result<WireRequestBody> {
                    Ok(WireRequestBody {
                        required: body
                            .required
                            .ok_or_else(|| anyhow!("v4 request body requiredness must be known"))?,
                        content: intern_content(&body.content, &mut schemas)?,
                    })
                })
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
        .map(|(content_type, schema)| {
            Ok((
                canonical_media_type(content_type)?,
                intern_schema(schema, schemas)?,
            ))
        })
        .collect()
}
fn intern_schema(schema: &Schema, schemas: &mut BTreeMap<String, WireSchema>) -> Result<String> {
    if matches!(&schema.additional_properties, AdditionalProperties::Unknown) {
        return Err(anyhow!("v4 additionalProperties policy must be known"));
    }
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
    let mut enum_values = schema.enum_values.clone();
    enum_values.sort();
    enum_values.dedup();
    let additional_properties = match &schema.additional_properties {
        AdditionalProperties::Unknown => unreachable!("v4 rejects unknown policies before hashing"),
        AdditionalProperties::Forbidden => WireAdditionalProperties::Forbidden,
        AdditionalProperties::Any => WireAdditionalProperties::Any,
        AdditionalProperties::Schema(schema) => WireAdditionalProperties::Schema {
            schema: intern_schema(schema, schemas)?,
        },
    };
    let wire = WireSchema {
        kind: schema.kind.clone(),
        nullable: schema.nullable,
        format: schema.format.clone(),
        enum_values,
        properties,
        additional_properties,
    };
    let id = canonical::schema_id(&wire)?;
    if let Some(existing) = schemas.get(&id) {
        if existing != &wire {
            return Err(anyhow!("schema digest collision"));
        }
    } else {
        schemas.insert(id.clone(), wire);
    }
    Ok(id)
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
                .map(|body| -> Result<RequestBody> {
                    Ok(RequestBody {
                        required: Some(body.required),
                        content: expand_content(&body.content, &contract.schemas)?,
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
        additional_properties: match &wire.additional_properties {
            WireAdditionalProperties::Forbidden => AdditionalProperties::Forbidden,
            WireAdditionalProperties::Any => AdditionalProperties::Any,
            WireAdditionalProperties::Schema { schema } => {
                AdditionalProperties::Schema(Box::new(expand_schema(schema, schemas)?))
            }
        },
    })
}
pub(super) fn validate_schema_table(contract: &Contract) -> Result<()> {
    let mut roots = Vec::new();
    for operation in contract.operations.values() {
        roots.extend(
            operation
                .parameters
                .values()
                .map(|parameter| parameter.schema.clone()),
        );
        if let Some(body) = &operation.request_body {
            roots.extend(body.content.values().cloned());
        }
        for content in operation.responses.values() {
            roots.extend(content.values().cloned());
        }
    }
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
        return Err(anyhow!("cyclic v4 schema reference"));
    }
    let schema = schemas
        .get(id)
        .ok_or_else(|| anyhow!("missing schema reference {}", sanitized(id)))?;
    if canonical::schema_id(schema)? != id {
        return Err(anyhow!("schema digest mismatch {}", sanitized(id)));
    }
    for property in schema.properties.values() {
        visit_schema(&property.schema, schemas, visiting, reachable)?;
    }
    if let WireAdditionalProperties::Schema { schema } = &schema.additional_properties {
        visit_schema(schema, schemas, visiting, reachable)?;
    }
    visiting.remove(id);
    reachable.insert(id.to_string());
    Ok(())
}
pub(super) fn parse_operation_key(value: &str) -> Result<OperationKey> {
    let (method, path) = value
        .split_once(' ')
        .ok_or_else(|| anyhow!("invalid v4 operation key"))?;
    if path.is_empty()
        || !path.starts_with('/')
        || method.is_empty()
        || path.chars().any(char::is_control)
        || method.chars().any(char::is_control)
        || path.chars().any(char::is_whitespace)
    {
        return Err(anyhow!("invalid v4 operation key"));
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
        _ => return Err(anyhow!("unsupported v4 operation method")),
    };
    Ok(OperationKey {
        method,
        path: path.to_string(),
    })
}
pub(super) fn parse_parameter_key(value: &str) -> Result<ParameterKey> {
    let (location, name) = value
        .split_once(':')
        .ok_or_else(|| anyhow!("invalid v4 parameter key"))?;
    if name.is_empty() || name.chars().any(char::is_control) {
        return Err(anyhow!("invalid v4 parameter key"));
    }
    let location = match location {
        "path" => ParameterLocation::Path,
        "query" => ParameterLocation::Query,
        "header" => ParameterLocation::Header,
        "cookie" => ParameterLocation::Cookie,
        _ => return Err(anyhow!("unsupported v4 parameter location")),
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
