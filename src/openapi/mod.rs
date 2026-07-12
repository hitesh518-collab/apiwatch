use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use openapiv3::{
    Components, IntegerFormat, MediaType, NumberFormat, OpenAPI, Operation as OpenApiOperation,
    Parameter as OpenApiParameter, ParameterData, ParameterSchemaOrContent, PathItem, ReferenceOr,
    RequestBody as OpenApiRequestBody, Response as OpenApiResponse, Schema as OpenApiSchema,
    SchemaKind as OpenApiSchemaKind, SecurityRequirement, SecurityScheme as OpenApiSecurityScheme,
    StatusCode, StringFormat, Type, VariantOrUnknownOrEmpty,
};

use crate::contract::{
    ApiContract, AuthRequirement, AuthSchemeKind, HttpMethod, Operation, OperationKey, Parameter,
    ParameterKey, ParameterLocation, Property, RequestBody, Response, Schema, SchemaKind,
};

pub fn load_contract(path: &Path) -> Result<ApiContract> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read OpenAPI file {}", path.display()))?;
    let document: OpenAPI = if path.extension().and_then(|value| value.to_str()) == Some("json") {
        serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse OpenAPI JSON {}", path.display()))?
    } else {
        serde_yaml::from_str(&raw)
            .with_context(|| format!("failed to parse OpenAPI YAML {}", path.display()))?
    };

    ensure_openapi_3(&document)?;

    normalize(document)
}

fn ensure_openapi_3(document: &OpenAPI) -> Result<()> {
    if document.openapi.starts_with("3.") {
        return Ok(());
    }

    Err(anyhow!(
        "unsupported OpenAPI version {}; expected OpenAPI 3.x",
        document.openapi
    ))
}

fn normalize(document: OpenAPI) -> Result<ApiContract> {
    let mut contract = ApiContract::new();
    let schema_resolver = SchemaResolver::from_components(document.components.as_ref());
    let security_schemes = normalize_security_schemes(document.components.as_ref())?;
    let global_security = document.security.clone().unwrap_or_default();
    let context = OperationNormalizeContext {
        security_schemes: &security_schemes,
        schema_resolver: &schema_resolver,
        global_security: &global_security,
    };
    let path_items = document
        .paths
        .paths
        .iter()
        .map(|(path, item)| (path.clone(), item.clone()))
        .collect::<BTreeMap<_, _>>();

    for (path, item) in document.paths.paths {
        let path = normalized_openapi_path(&path)?;
        let item = resolve_path_item(&item, &path_items, &mut BTreeSet::new())?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Get,
            &context,
            &item.parameters,
            item.get.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Post,
            &context,
            &item.parameters,
            item.post.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Put,
            &context,
            &item.parameters,
            item.put.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Patch,
            &context,
            &item.parameters,
            item.patch.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Delete,
            &context,
            &item.parameters,
            item.delete.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Options,
            &context,
            &item.parameters,
            item.options.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Head,
            &context,
            &item.parameters,
            item.head.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Trace,
            &context,
            &item.parameters,
            item.trace.as_ref(),
        )?;
    }

    Ok(contract)
}

fn normalized_openapi_path(path: &str) -> Result<&str> {
    if path.is_empty() {
        return Err(anyhow!("OpenAPI path cannot be empty"));
    }

    if !path.starts_with('/') {
        return Err(anyhow!("OpenAPI path must start with /"));
    }

    if path.chars().any(char::is_control) {
        return Err(anyhow!("OpenAPI path contains a control character"));
    }

    Ok(path)
}

struct OperationNormalizeContext<'a> {
    security_schemes: &'a BTreeMap<String, AuthSchemeKind>,
    schema_resolver: &'a SchemaResolver,
    global_security: &'a [SecurityRequirement],
}

fn resolve_path_item(
    item: &ReferenceOr<PathItem>,
    path_items: &BTreeMap<String, ReferenceOr<PathItem>>,
    visiting: &mut BTreeSet<String>,
) -> Result<PathItem> {
    match item {
        ReferenceOr::Item(item) => Ok(item.clone()),
        ReferenceOr::Reference { reference } => {
            let path = path_item_reference_path(reference)?;
            if !visiting.insert(path.clone()) {
                return Err(anyhow!(
                    "circular path item reference detected: {reference}"
                ));
            }

            let item = path_items
                .get(&path)
                .ok_or_else(|| anyhow!("path item reference not found: {reference}"))?;
            let resolved = resolve_path_item(item, path_items, visiting);
            visiting.remove(&path);
            resolved
        }
    }
}

fn path_item_reference_path(reference: &str) -> Result<String> {
    component_name(reference, "#/paths/", "path item")
}

fn insert_operation(
    contract: &mut ApiContract,
    path: &str,
    method: HttpMethod,
    context: &OperationNormalizeContext<'_>,
    path_parameters: &[ReferenceOr<OpenApiParameter>],
    operation: Option<&OpenApiOperation>,
) -> Result<()> {
    let Some(operation) = operation else {
        return Ok(());
    };

    let auth = normalize_auth_requirements(
        operation
            .security
            .as_deref()
            .unwrap_or(context.global_security),
        context.security_schemes,
    );

    let parameters = normalize_parameters(
        context.schema_resolver,
        path_parameters,
        &operation.parameters,
    )?;

    let request_body = operation
        .request_body
        .as_ref()
        .map(|request_body| normalize_request_body(request_body, context.schema_resolver))
        .transpose()?;

    let mut responses = BTreeMap::new();
    for (status, response) in &operation.responses.responses {
        let status = normalize_status_code(status);
        let response = normalize_response(response, context.schema_resolver)?;
        responses.insert(status, response);
    }

    contract.operations.insert(
        OperationKey {
            method,
            path: path.to_string(),
        },
        Operation {
            auth,
            parameters,
            request_body,
            responses,
        },
    );

    Ok(())
}

fn normalize_status_code(status: &StatusCode) -> String {
    match status {
        StatusCode::Code(_) | StatusCode::Range(_) => status.to_string(),
    }
}

fn normalize_security_schemes(
    components: Option<&Components>,
) -> Result<BTreeMap<String, AuthSchemeKind>> {
    let mut schemes = BTreeMap::new();

    let Some(components) = components else {
        return Ok(schemes);
    };

    let security_schemes = components
        .security_schemes
        .iter()
        .map(|(name, scheme)| (name.clone(), scheme.clone()))
        .collect::<BTreeMap<_, _>>();

    for (name, scheme) in &security_schemes {
        let kind = normalize_security_scheme_ref(scheme, &security_schemes, &mut BTreeSet::new())?;
        schemes.insert(name.clone(), kind);
    }

    Ok(schemes)
}

fn normalize_security_scheme_ref(
    scheme: &ReferenceOr<OpenApiSecurityScheme>,
    security_schemes: &BTreeMap<String, ReferenceOr<OpenApiSecurityScheme>>,
    visiting: &mut BTreeSet<String>,
) -> Result<AuthSchemeKind> {
    match scheme {
        ReferenceOr::Item(scheme) => Ok(auth_scheme_kind(scheme)),
        ReferenceOr::Reference { reference } => {
            resolve_security_scheme(reference, security_schemes, visiting)
        }
    }
}

fn resolve_security_scheme(
    reference: &str,
    security_schemes: &BTreeMap<String, ReferenceOr<OpenApiSecurityScheme>>,
    visiting: &mut BTreeSet<String>,
) -> Result<AuthSchemeKind> {
    let name = component_name(
        reference,
        "#/components/securitySchemes/",
        "security scheme",
    )?;
    if !visiting.insert(name.clone()) {
        return Err(anyhow!(
            "circular security scheme reference detected: {reference}"
        ));
    }

    let scheme = security_schemes
        .get(&name)
        .ok_or_else(|| anyhow!("security scheme reference not found: {reference}"))?;
    let kind = normalize_security_scheme_ref(scheme, security_schemes, visiting);
    visiting.remove(&name);
    kind
}

fn auth_scheme_kind(scheme: &OpenApiSecurityScheme) -> AuthSchemeKind {
    match scheme {
        OpenApiSecurityScheme::APIKey { .. } => AuthSchemeKind::ApiKey,
        OpenApiSecurityScheme::HTTP { scheme, .. } => {
            if scheme.eq_ignore_ascii_case("bearer") {
                AuthSchemeKind::Bearer
            } else if scheme.eq_ignore_ascii_case("basic") {
                AuthSchemeKind::Basic
            } else {
                AuthSchemeKind::Http
            }
        }
        OpenApiSecurityScheme::OAuth2 { .. } => AuthSchemeKind::OAuth2,
        OpenApiSecurityScheme::OpenIDConnect { .. } => AuthSchemeKind::OpenIdConnect,
    }
}

fn normalize_auth_requirements(
    requirements: &[SecurityRequirement],
    security_schemes: &BTreeMap<String, AuthSchemeKind>,
) -> BTreeMap<String, AuthRequirement> {
    let mut auth = BTreeMap::new();

    if requirements
        .iter()
        .any(|requirement| requirement.is_empty())
    {
        return auth;
    }

    for requirement in requirements {
        for (name, scopes) in requirement {
            let mut scopes = scopes.clone();
            scopes.sort();

            auth.insert(
                name.clone(),
                AuthRequirement {
                    name: name.clone(),
                    kind: security_schemes
                        .get(name)
                        .copied()
                        .unwrap_or(AuthSchemeKind::Unknown),
                    scopes,
                },
            );
        }
    }

    auth
}

struct SchemaResolver {
    parameters: BTreeMap<String, ReferenceOr<OpenApiParameter>>,
    request_bodies: BTreeMap<String, ReferenceOr<OpenApiRequestBody>>,
    responses: BTreeMap<String, ReferenceOr<OpenApiResponse>>,
    schemas: BTreeMap<String, ReferenceOr<OpenApiSchema>>,
}

impl SchemaResolver {
    fn from_components(components: Option<&Components>) -> Self {
        let parameters = components
            .map(|components| {
                components
                    .parameters
                    .iter()
                    .map(|(name, parameter)| (name.clone(), parameter.clone()))
                    .collect()
            })
            .unwrap_or_default();

        let request_bodies = components
            .map(|components| {
                components
                    .request_bodies
                    .iter()
                    .map(|(name, request_body)| (name.clone(), request_body.clone()))
                    .collect()
            })
            .unwrap_or_default();

        let responses = components
            .map(|components| {
                components
                    .responses
                    .iter()
                    .map(|(name, response)| (name.clone(), response.clone()))
                    .collect()
            })
            .unwrap_or_default();

        let schemas = components
            .map(|components| {
                components
                    .schemas
                    .iter()
                    .map(|(name, schema)| (name.clone(), schema.clone()))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            parameters,
            request_bodies,
            responses,
            schemas,
        }
    }

    fn resolve_parameter(
        &self,
        reference: &str,
        visiting: &mut BTreeSet<String>,
    ) -> Result<(ParameterKey, Parameter)> {
        let name = component_name(reference, "#/components/parameters/", "parameter")?;
        if !visiting.insert(name.clone()) {
            return Err(anyhow!(
                "circular parameter reference detected: {reference}"
            ));
        }

        let parameter = self
            .parameters
            .get(&name)
            .ok_or_else(|| anyhow!("parameter reference not found: {reference}"))?;
        let normalized = normalize_parameter_ref(parameter, self, visiting);
        visiting.remove(&name);
        normalized
    }

    fn resolve_request_body(
        &self,
        reference: &str,
        visiting: &mut BTreeSet<String>,
    ) -> Result<RequestBody> {
        let name = component_name(reference, "#/components/requestBodies/", "request body")?;
        if !visiting.insert(name.clone()) {
            return Err(anyhow!(
                "circular request body reference detected: {reference}"
            ));
        }

        let request_body = self
            .request_bodies
            .get(&name)
            .ok_or_else(|| anyhow!("request body reference not found: {reference}"))?;
        let normalized = normalize_request_body_ref(request_body, self, visiting);
        visiting.remove(&name);
        normalized
    }

    fn resolve_response(
        &self,
        reference: &str,
        visiting: &mut BTreeSet<String>,
    ) -> Result<Response> {
        let name = component_name(reference, "#/components/responses/", "response")?;
        if !visiting.insert(name.clone()) {
            return Err(anyhow!("circular response reference detected: {reference}"));
        }

        let response = self
            .responses
            .get(&name)
            .ok_or_else(|| anyhow!("response reference not found: {reference}"))?;
        let normalized = normalize_response_ref(response, self, visiting);
        visiting.remove(&name);
        normalized
    }

    fn resolve(&self, reference: &str, visiting: &mut BTreeSet<String>) -> Result<Schema> {
        let name = component_name(reference, "#/components/schemas/", "schema")?;
        if !visiting.insert(name.clone()) {
            return Err(anyhow!("circular schema reference detected: {reference}"));
        }

        let schema = self
            .schemas
            .get(&name)
            .ok_or_else(|| anyhow!("schema reference not found: {reference}"))?;
        let normalized = normalize_schema_ref(schema, self, visiting);
        visiting.remove(&name);
        normalized
    }
}

fn component_name(reference: &str, prefix: &str, kind: &str) -> Result<String> {
    let name = reference
        .strip_prefix(prefix)
        .ok_or_else(|| anyhow!("unsupported {kind} reference: {reference}"))?;

    Ok(decode_json_pointer_token(name))
}

fn decode_json_pointer_token(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

fn normalize_parameters(
    schema_resolver: &SchemaResolver,
    path_parameters: &[ReferenceOr<OpenApiParameter>],
    operation_parameters: &[ReferenceOr<OpenApiParameter>],
) -> Result<BTreeMap<ParameterKey, Parameter>> {
    let mut parameters = BTreeMap::new();

    for parameter in path_parameters {
        let (key, parameter) =
            normalize_parameter_ref(parameter, schema_resolver, &mut BTreeSet::new())?;
        parameters.insert(key, parameter);
    }

    for parameter in operation_parameters {
        let (key, parameter) =
            normalize_parameter_ref(parameter, schema_resolver, &mut BTreeSet::new())?;
        parameters.insert(key, parameter);
    }

    Ok(parameters)
}

fn normalize_parameter_ref(
    parameter: &ReferenceOr<OpenApiParameter>,
    schema_resolver: &SchemaResolver,
    visiting_parameters: &mut BTreeSet<String>,
) -> Result<(ParameterKey, Parameter)> {
    let parameter = match parameter {
        ReferenceOr::Item(parameter) => parameter,
        ReferenceOr::Reference { reference } => {
            return schema_resolver.resolve_parameter(reference, visiting_parameters);
        }
    };

    let (location, data) = parameter_location_and_data(parameter);
    let schema = normalize_parameter_schema(data, schema_resolver)?;
    let key_name = normalize_parameter_key_name(location, &data.name);

    Ok((
        ParameterKey {
            location,
            name: key_name,
        },
        Parameter {
            name: data.name.clone(),
            required: data.required || location == ParameterLocation::Path,
            schema,
        },
    ))
}

fn parameter_location_and_data(
    parameter: &OpenApiParameter,
) -> (ParameterLocation, &ParameterData) {
    match parameter {
        OpenApiParameter::Query { parameter_data, .. } => {
            (ParameterLocation::Query, parameter_data)
        }
        OpenApiParameter::Header { parameter_data, .. } => {
            (ParameterLocation::Header, parameter_data)
        }
        OpenApiParameter::Path { parameter_data, .. } => (ParameterLocation::Path, parameter_data),
        OpenApiParameter::Cookie { parameter_data, .. } => {
            (ParameterLocation::Cookie, parameter_data)
        }
    }
}

fn normalize_parameter_key_name(location: ParameterLocation, name: &str) -> String {
    if location == ParameterLocation::Header {
        name.to_ascii_lowercase()
    } else {
        name.to_string()
    }
}

fn normalize_parameter_schema(
    data: &ParameterData,
    schema_resolver: &SchemaResolver,
) -> Result<Schema> {
    match &data.format {
        ParameterSchemaOrContent::Schema(schema) => {
            normalize_schema_ref(schema, schema_resolver, &mut BTreeSet::new())
        }
        ParameterSchemaOrContent::Content(content) => {
            let Some((_, media_type)) = content.first() else {
                return Ok(unknown_schema());
            };
            normalize_media_type(media_type, schema_resolver)
        }
    }
}

fn normalize_request_body(
    request_body: &ReferenceOr<OpenApiRequestBody>,
    schema_resolver: &SchemaResolver,
) -> Result<RequestBody> {
    normalize_request_body_ref(request_body, schema_resolver, &mut BTreeSet::new())
}

fn normalize_request_body_ref(
    request_body: &ReferenceOr<OpenApiRequestBody>,
    schema_resolver: &SchemaResolver,
    visiting_request_bodies: &mut BTreeSet<String>,
) -> Result<RequestBody> {
    let request_body = match request_body {
        ReferenceOr::Item(request_body) => request_body,
        ReferenceOr::Reference { reference } => {
            return schema_resolver.resolve_request_body(reference, visiting_request_bodies);
        }
    };

    let mut content = BTreeMap::new();
    for (content_type, media_type) in &request_body.content {
        content.insert(
            content_type.clone(),
            normalize_media_type(media_type, schema_resolver)?,
        );
    }

    Ok(RequestBody { content })
}

fn normalize_response(
    response: &ReferenceOr<OpenApiResponse>,
    schema_resolver: &SchemaResolver,
) -> Result<Response> {
    normalize_response_ref(response, schema_resolver, &mut BTreeSet::new())
}

fn normalize_response_ref(
    response: &ReferenceOr<OpenApiResponse>,
    schema_resolver: &SchemaResolver,
    visiting_responses: &mut BTreeSet<String>,
) -> Result<Response> {
    let response = match response {
        ReferenceOr::Item(response) => response,
        ReferenceOr::Reference { reference } => {
            return schema_resolver.resolve_response(reference, visiting_responses);
        }
    };

    let mut content = BTreeMap::new();
    for (content_type, media_type) in &response.content {
        content.insert(
            content_type.clone(),
            normalize_media_type(media_type, schema_resolver)?,
        );
    }

    Ok(Response { content })
}

fn normalize_media_type(
    media_type: &MediaType,
    schema_resolver: &SchemaResolver,
) -> Result<Schema> {
    match &media_type.schema {
        Some(schema) => normalize_schema_ref(schema, schema_resolver, &mut BTreeSet::new()),
        None => Ok(unknown_schema()),
    }
}

fn normalize_schema_ref(
    schema: &ReferenceOr<OpenApiSchema>,
    schema_resolver: &SchemaResolver,
    visiting: &mut BTreeSet<String>,
) -> Result<Schema> {
    match schema {
        ReferenceOr::Item(schema) => normalize_schema(schema, schema_resolver, visiting),
        ReferenceOr::Reference { reference } => schema_resolver.resolve(reference, visiting),
    }
}

fn normalize_boxed_schema_ref(
    schema: &ReferenceOr<Box<OpenApiSchema>>,
    schema_resolver: &SchemaResolver,
    visiting: &mut BTreeSet<String>,
) -> Result<Schema> {
    match schema {
        ReferenceOr::Item(schema) => normalize_schema(schema.as_ref(), schema_resolver, visiting),
        ReferenceOr::Reference { reference } => schema_resolver.resolve(reference, visiting),
    }
}

fn normalize_schema(
    schema: &OpenApiSchema,
    schema_resolver: &SchemaResolver,
    visiting: &mut BTreeSet<String>,
) -> Result<Schema> {
    let mut normalized = unknown_schema();
    normalized.nullable = schema.schema_data.nullable;

    match &schema.schema_kind {
        OpenApiSchemaKind::Type(Type::Object(object)) => {
            normalized.kind = SchemaKind::Object;
            normalized.properties = object
                .properties
                .iter()
                .map(|(name, schema)| {
                    let required = object.required.iter().any(|candidate| candidate == name);
                    let schema = normalize_boxed_schema_ref(schema, schema_resolver, visiting)?;
                    Ok((
                        name.clone(),
                        Property {
                            required,
                            schema: Box::new(schema),
                        },
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?;
        }
        OpenApiSchemaKind::Type(Type::Array(array)) => {
            normalized.kind = SchemaKind::Array;
            if let Some(items) = &array.items {
                normalized.properties.insert(
                    "items".to_string(),
                    Property {
                        required: true,
                        schema: Box::new(normalize_boxed_schema_ref(
                            items,
                            schema_resolver,
                            visiting,
                        )?),
                    },
                );
            }
        }
        OpenApiSchemaKind::OneOf { one_of } => {
            normalized.kind = SchemaKind::OneOf;
            normalized.properties =
                normalize_composed_schema_refs("oneOf", one_of, schema_resolver, visiting)?;
        }
        OpenApiSchemaKind::AllOf { all_of } => {
            normalized.kind = SchemaKind::AllOf;
            normalized.properties =
                normalize_composed_schema_refs("allOf", all_of, schema_resolver, visiting)?;
        }
        OpenApiSchemaKind::AnyOf { any_of } => {
            normalized.kind = SchemaKind::AnyOf;
            normalized.properties =
                normalize_composed_schema_refs("anyOf", any_of, schema_resolver, visiting)?;
        }
        OpenApiSchemaKind::Type(Type::String(string)) => {
            normalized.kind = SchemaKind::String;
            normalized.format = string_format_name(&string.format);
            normalized.enum_values = string.enumeration.iter().flatten().cloned().collect();
        }
        OpenApiSchemaKind::Type(Type::Integer(integer)) => {
            normalized.kind = SchemaKind::Integer;
            normalized.format = integer_format_name(&integer.format);
            normalized.enum_values = integer
                .enumeration
                .iter()
                .flatten()
                .map(|value| value.to_string())
                .collect();
        }
        OpenApiSchemaKind::Type(Type::Number(number)) => {
            normalized.kind = SchemaKind::Number;
            normalized.format = number_format_name(&number.format);
            normalized.enum_values = number
                .enumeration
                .iter()
                .flatten()
                .map(|value| value.to_string())
                .collect();
        }
        OpenApiSchemaKind::Type(Type::Boolean(_)) => {
            normalized.kind = SchemaKind::Boolean;
        }
        _ => {
            normalized.kind = SchemaKind::Unknown;
        }
    }

    Ok(normalized)
}

fn normalize_composed_schema_refs(
    prefix: &str,
    schemas: &[ReferenceOr<OpenApiSchema>],
    schema_resolver: &SchemaResolver,
    visiting: &mut BTreeSet<String>,
) -> Result<BTreeMap<String, Property>> {
    schemas
        .iter()
        .enumerate()
        .map(|(index, schema)| {
            let schema = normalize_schema_ref(schema, schema_resolver, visiting)?;
            Ok((
                format!("{prefix}[{index}]"),
                Property {
                    required: true,
                    schema: Box::new(schema),
                },
            ))
        })
        .collect()
}

fn string_format_name(format: &VariantOrUnknownOrEmpty<StringFormat>) -> Option<String> {
    match format {
        VariantOrUnknownOrEmpty::Item(StringFormat::Date) => Some("date".to_string()),
        VariantOrUnknownOrEmpty::Item(StringFormat::DateTime) => Some("date-time".to_string()),
        VariantOrUnknownOrEmpty::Item(StringFormat::Password) => Some("password".to_string()),
        VariantOrUnknownOrEmpty::Item(StringFormat::Byte) => Some("byte".to_string()),
        VariantOrUnknownOrEmpty::Item(StringFormat::Binary) => Some("binary".to_string()),
        VariantOrUnknownOrEmpty::Unknown(format) => Some(format.clone()),
        VariantOrUnknownOrEmpty::Empty => None,
    }
}

fn integer_format_name(format: &VariantOrUnknownOrEmpty<IntegerFormat>) -> Option<String> {
    match format {
        VariantOrUnknownOrEmpty::Item(IntegerFormat::Int32) => Some("int32".to_string()),
        VariantOrUnknownOrEmpty::Item(IntegerFormat::Int64) => Some("int64".to_string()),
        VariantOrUnknownOrEmpty::Unknown(format) => Some(format.clone()),
        VariantOrUnknownOrEmpty::Empty => None,
    }
}

fn number_format_name(format: &VariantOrUnknownOrEmpty<NumberFormat>) -> Option<String> {
    match format {
        VariantOrUnknownOrEmpty::Item(NumberFormat::Float) => Some("float".to_string()),
        VariantOrUnknownOrEmpty::Item(NumberFormat::Double) => Some("double".to_string()),
        VariantOrUnknownOrEmpty::Unknown(format) => Some(format.clone()),
        VariantOrUnknownOrEmpty::Empty => None,
    }
}

fn unknown_schema() -> Schema {
    Schema {
        kind: SchemaKind::Unknown,
        nullable: false,
        format: None,
        enum_values: Vec::new(),
        properties: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::contract::HttpMethod;

    use super::load_contract;

    #[test]
    fn loads_openapi_operations() {
        let contract = load_contract(Path::new("testdata/openapi/endpoint_removed_old.yaml"))
            .expect("fixture should parse");

        let key = contract
            .operations
            .keys()
            .find(|key| key.path == "/users" && key.method == HttpMethod::Get)
            .expect("GET /users should be normalized");

        let operation = contract
            .operations
            .get(key)
            .expect("operation should exist");
        assert!(operation.responses.contains_key("200"));
    }
}
