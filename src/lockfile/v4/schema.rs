use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};

use super::{
    canonical, Contract, WireAdditionalProperties, WireAuth, WireAuthIdentity,
    WireOAuthFlowIdentity, WireOAuthFlowKind, WireOperation, WireParameter, WireProperty,
    WireRequestBody, WireSchema,
};
use crate::contract::{
    AdditionalProperties, ApiContract, AuthIdentity, AuthRequirement, HttpMethod,
    OAuthFlowIdentity, OAuthFlowKind, Operation, OperationIdentity, OperationKey, Parameter,
    ParameterKey, ParameterLocation, Property, RequestBody, Response, Schema, ServerTemplate,
};
use crate::openapi::identity::{canonical_media_type, canonical_path_template};

pub(super) fn intern_contract(contract: &ApiContract) -> Result<Contract> {
    let mut schemas = BTreeMap::new();
    let operations = contract
        .operations
        .iter()
        .map(|(key, operation)| {
            let auth = operation
                .auth
                .iter()
                .map(|(name, requirement)| -> Result<_> {
                    let mut scopes = requirement.scopes.clone();
                    scopes.sort();
                    scopes.dedup();
                    Ok((
                        name.clone(),
                        WireAuth {
                            kind: requirement.kind,
                            identity: intern_auth_identity(
                                requirement
                                    .identity
                                    .as_ref()
                                    .ok_or_else(|| anyhow!("v4 auth identity must be known"))?,
                            ),
                            scopes,
                        },
                    ))
                })
                .collect::<Result<_>>()?;
            let servers = operation
                .servers
                .as_ref()
                .ok_or_else(|| anyhow!("v4 server data must be known"))?
                .iter()
                .map(|server| server.0.clone())
                .collect();
            let parameters = operation
                .parameters
                .iter()
                .map(|(key, parameter)| {
                    Ok((
                        format!("{}:{}", key.location.as_str(), key.name),
                        WireParameter {
                            name: parameter.name.clone(),
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
                    display_path: operation.key.path.clone(),
                    auth,
                    servers,
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

fn intern_auth_identity(identity: &AuthIdentity) -> WireAuthIdentity {
    match identity {
        AuthIdentity::ApiKey { location, name } => WireAuthIdentity::ApiKey {
            location: *location,
            name: name.clone(),
        },
        AuthIdentity::Http { scheme } => WireAuthIdentity::Http {
            scheme: scheme.clone(),
        },
        AuthIdentity::OAuth2 { flows } => WireAuthIdentity::OAuth2 {
            flows: flows
                .iter()
                .map(|flow| WireOAuthFlowIdentity {
                    kind: match flow.kind {
                        OAuthFlowKind::Implicit => WireOAuthFlowKind::Implicit,
                        OAuthFlowKind::Password => WireOAuthFlowKind::Password,
                        OAuthFlowKind::ClientCredentials => WireOAuthFlowKind::ClientCredentials,
                        OAuthFlowKind::AuthorizationCode => WireOAuthFlowKind::AuthorizationCode,
                    },
                    authorization: flow
                        .authorization
                        .as_ref()
                        .map(|endpoint| endpoint.0.clone()),
                    token: flow.token.as_ref().map(|endpoint| endpoint.0.clone()),
                    refresh: flow.refresh.as_ref().map(|endpoint| endpoint.0.clone()),
                })
                .collect(),
        },
        AuthIdentity::OpenIdConnect { discovery } => WireAuthIdentity::OpenIdConnect {
            discovery: discovery.0.clone(),
        },
        AuthIdentity::Unknown { kind } => WireAuthIdentity::Unknown { kind: *kind },
    }
}

fn expand_auth_identity(identity: &WireAuthIdentity) -> AuthIdentity {
    match identity {
        WireAuthIdentity::ApiKey { location, name } => AuthIdentity::ApiKey {
            location: *location,
            name: name.clone(),
        },
        WireAuthIdentity::Http { scheme } => AuthIdentity::Http {
            scheme: scheme.clone(),
        },
        WireAuthIdentity::OAuth2 { flows } => AuthIdentity::OAuth2 {
            flows: flows
                .iter()
                .map(|flow| OAuthFlowIdentity {
                    kind: match flow.kind {
                        WireOAuthFlowKind::Implicit => OAuthFlowKind::Implicit,
                        WireOAuthFlowKind::Password => OAuthFlowKind::Password,
                        WireOAuthFlowKind::ClientCredentials => OAuthFlowKind::ClientCredentials,
                        WireOAuthFlowKind::AuthorizationCode => OAuthFlowKind::AuthorizationCode,
                    },
                    authorization: flow
                        .authorization
                        .as_ref()
                        .map(|endpoint| ServerTemplate(endpoint.clone())),
                    token: flow
                        .token
                        .as_ref()
                        .map(|endpoint| ServerTemplate(endpoint.clone())),
                    refresh: flow
                        .refresh
                        .as_ref()
                        .map(|endpoint| ServerTemplate(endpoint.clone())),
                })
                .collect(),
        },
        WireAuthIdentity::OpenIdConnect { discovery } => AuthIdentity::OpenIdConnect {
            discovery: ServerTemplate(discovery.clone()),
        },
        WireAuthIdentity::Unknown { kind } => AuthIdentity::Unknown { kind: *kind },
    }
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
    let items = schema
        .items
        .as_ref()
        .map(|items| intern_schema(items, schemas))
        .transpose()?;
    if !matches!(
        schema.kind,
        crate::contract::SchemaKind::OneOf | crate::contract::SchemaKind::AnyOf
    ) && !schema.branches.is_empty()
    {
        return Err(anyhow!(
            "v4 branches are only valid for oneOf or anyOf schemas"
        ));
    }
    let mut branches = schema
        .branches
        .iter()
        .map(|branch| intern_schema(branch, schemas))
        .collect::<Result<Vec<_>>>()?;
    branches.sort();
    branches.dedup();
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
        items,
        additional_properties,
        branches,
        cycle_target: schema.cycle_target.clone(),
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
            let identity = parse_operation_key(key)?;
            let (canonical_display_path, _) = canonical_path_template(&operation.display_path)?;
            if canonical_display_path != identity.path {
                return Err(anyhow!(
                    "v4 operation display path does not match its canonical identity"
                ));
            }
            let key = OperationKey {
                method: identity.method,
                path: operation.display_path.clone(),
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
                            identity: Some(expand_auth_identity(&requirement.identity)),
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
                            name: parameter.name.clone(),
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
                identity,
                Operation {
                    key,
                    auth,
                    servers: Some(
                        operation
                            .servers
                            .iter()
                            .map(|server| Ok(crate::contract::ServerTemplate(server.clone())))
                            .collect::<Result<_>>()?,
                    ),
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
    let items = wire
        .items
        .as_ref()
        .map(|items| expand_schema(items, schemas).map(Box::new))
        .transpose()?;
    if !matches!(
        wire.kind,
        crate::contract::SchemaKind::OneOf | crate::contract::SchemaKind::AnyOf
    ) && !wire.branches.is_empty()
    {
        return Err(anyhow!(
            "v4 branches are only valid for oneOf or anyOf schemas"
        ));
    }
    let mut branches: Vec<Schema> = wire
        .branches
        .iter()
        .map(|branch| expand_schema(branch, schemas))
        .collect::<Result<_>>()?;
    branches.sort_by_key(Schema::structural_key);
    branches.dedup_by(|left, right| left.structural_key() == right.structural_key());
    Ok(Schema {
        kind: wire.kind.clone(),
        nullable: wire.nullable,
        format: wire.format.clone(),
        enum_values: wire.enum_values.clone(),
        properties,
        items,
        additional_properties: match &wire.additional_properties {
            WireAdditionalProperties::Forbidden => AdditionalProperties::Forbidden,
            WireAdditionalProperties::Any => AdditionalProperties::Any,
            WireAdditionalProperties::Schema { schema } => {
                AdditionalProperties::Schema(Box::new(expand_schema(schema, schemas)?))
            }
        },
        branches,
        cycle_target: wire.cycle_target.clone(),
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
    if let Some(items) = &schema.items {
        visit_schema(items, schemas, visiting, reachable)?;
    }
    for branch in &schema.branches {
        visit_schema(branch, schemas, visiting, reachable)?;
    }
    if let WireAdditionalProperties::Schema { schema } = &schema.additional_properties {
        visit_schema(schema, schemas, visiting, reachable)?;
    }
    visiting.remove(id);
    reachable.insert(id.to_string());
    Ok(())
}
pub(super) fn parse_operation_key(value: &str) -> Result<OperationIdentity> {
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
    let (path, _) = canonical_path_template(path)?;
    Ok(OperationIdentity { method, path })
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
