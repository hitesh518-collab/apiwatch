pub(crate) mod identity;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{anyhow, Context, Result};
use openapiv3::{
    AdditionalProperties as OpenApiAdditionalProperties, Components, IntegerFormat, MediaType,
    NumberFormat, OpenAPI, Operation as OpenApiOperation, Parameter as OpenApiParameter,
    ParameterData, ParameterSchemaOrContent, PathItem, ReferenceOr,
    RequestBody as OpenApiRequestBody, Response as OpenApiResponse, Schema as OpenApiSchema,
    SchemaKind as OpenApiSchemaKind, SecurityRequirement, SecurityScheme as OpenApiSecurityScheme,
    Server, StatusCode, StringFormat, Type, VariantOrUnknownOrEmpty,
};

use crate::contract::{
    AdditionalProperties, ApiContract, AuthIdentity, AuthRequirement, AuthSchemeKind, HttpMethod,
    OAuthFlowIdentity, OAuthFlowKind, Operation, OperationIdentity, OperationKey, Parameter,
    ParameterKey, ParameterLocation, Property, RequestBody, Response, Schema, SchemaKind,
};

pub fn load_contract(path: &Path) -> Result<ApiContract> {
    load_contract_with_ref_root(path, None)
}

pub fn load_contract_with_ref_root(path: &Path, ref_root: Option<PathBuf>) -> Result<ApiContract> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read OpenAPI file {}", path.display()))?;
    let is_json = path.extension().and_then(|value| value.to_str()) == Some("json");
    let computed_ref_root = ref_root.or_else(|| {
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
    });
    load_contract_text(
        &raw,
        is_json,
        path.to_string_lossy().as_ref(),
        computed_ref_root,
    )
}

pub fn load_contract_text(
    text: &str,
    is_json: bool,
    location: &str,
    ref_root: Option<PathBuf>,
) -> Result<ApiContract> {
    validate_raw_openapi(text, is_json)?;

    let document: OpenAPI = if is_json {
        serde_json::from_str(text)
            .with_context(|| format!("failed to parse OpenAPI JSON {location}"))?
    } else {
        let bytes = text.as_bytes();
        let mut value: serde_yml::Value = serde_yml::from_slice(bytes)
            .with_context(|| format!("failed to parse OpenAPI YAML {location}"))?;

        if is_openapi31(&value) {
            normalize_openapi31_to_30(&mut value);
            let normalized = serde_yml::to_string(&value)
                .with_context(|| "failed to serialize normalized OpenAPI 3.1")?;
            tolerant_openapi_yaml(normalized.as_bytes())
                .with_context(|| format!("failed to parse OpenAPI YAML {location}"))?
        } else {
            tolerant_openapi_yaml(bytes)
                .with_context(|| format!("failed to parse OpenAPI YAML {location}"))?
        }
    };

    ensure_openapi_3(&document)?;

    normalize(document, ref_root)
}

fn tolerant_openapi_yaml(bytes: &[u8]) -> Result<OpenAPI> {
    let mut value: serde_yml::Value =
        serde_yml::from_slice(bytes).context("failed to parse OpenAPI YAML")?;

    strip_deep(&mut value, "tags");
    strip_deep(&mut value, "externalDocs");
    strip_deep(&mut value, "examples");
    strip_deep(&mut value, "callbacks");
    strip_deep_by_prefix(&mut value, "x-");
    strip_broken_path_operations(&mut value);

    let cleaned = serde_yml::to_string(&value).context("failed to re-serialize OpenAPI YAML")?;
    serde_yml::from_str(&cleaned).context("failed to parse cleaned OpenAPI YAML")
}

fn strip_broken_path_operations(value: &mut serde_yml::Value) {
    const HTTP_METHODS: &[&str] = &[
        "get", "put", "post", "delete", "options", "head", "patch", "trace",
    ];
    let Some(paths) = value.get_mut("paths") else {
        return;
    };
    let serde_yml::Value::Mapping(paths_map) = paths else {
        return;
    };
    paths_map.retain(|_path_key, path_value| {
        let serde_yml::Value::Mapping(path_obj) = path_value else {
            return true;
        };
        path_obj.retain(|method_key, op_value| {
            let Some(method_str) = method_key.as_str() else {
                return true;
            };
            if !HTTP_METHODS.contains(&method_str) {
                return true;
            }
            let serde_yml::Value::Mapping(op_map) = op_value else {
                return false;
            };
            op_map.contains_key(serde_yml::Value::String("responses".into()))
        });
        !path_obj.is_empty()
    });
}

fn strip_deep(value: &mut serde_yml::Value, key: &str) {
    match value {
        serde_yml::Value::Mapping(map) => {
            map.retain(|k, v| {
                if k.as_str() == Some(key) {
                    return false;
                }
                strip_deep(v, key);
                true
            });
        }
        serde_yml::Value::Sequence(seq) => {
            for item in seq {
                strip_deep(item, key);
            }
        }
        _ => {}
    }
}

fn strip_deep_by_prefix(value: &mut serde_yml::Value, prefix: &str) {
    match value {
        serde_yml::Value::Mapping(map) => {
            map.retain(|k, v| {
                if k.as_str().is_some_and(|s| s.starts_with(prefix)) {
                    return false;
                }
                strip_deep_by_prefix(v, prefix);
                true
            });
        }
        serde_yml::Value::Sequence(seq) => {
            for item in seq {
                strip_deep_by_prefix(item, prefix);
            }
        }
        _ => {}
    }
}

fn is_openapi31(value: &serde_yml::Value) -> bool {
    value
        .get("openapi")
        .and_then(|v| v.as_str())
        .is_some_and(|v| v == "3.1" || v.starts_with("3.1."))
}

fn normalize_openapi31_to_30(value: &mut serde_yml::Value) {
    update_version(value);
    move_defs_to_components(value);
    move_webhooks_to_paths(value);
    transform_schema(value);
}

fn update_version(value: &mut serde_yml::Value) {
    let key = serde_yml::Value::String("openapi".to_string());
    if let serde_yml::Value::Mapping(map) = value {
        if let Some(v) = map.get_mut(&key) {
            *v = serde_yml::Value::String("3.0.3".to_string());
        }
    }
}

fn move_defs_to_components(value: &mut serde_yml::Value) {
    let defs_key = serde_yml::Value::String("$defs".to_string());
    let components_key = serde_yml::Value::String("components".to_string());
    let schemas_key = serde_yml::Value::String("schemas".to_string());

    if let serde_yml::Value::Mapping(map) = value {
        if let Some(serde_yml::Value::Mapping(defs_map)) = map.remove(&defs_key) {
            let components = map
                .entry(components_key)
                .or_insert_with(|| serde_yml::Value::Mapping(serde_yml::Mapping::new()));
            if let serde_yml::Value::Mapping(comp_map) = components {
                let schemas = comp_map
                    .entry(schemas_key)
                    .or_insert_with(|| serde_yml::Value::Mapping(serde_yml::Mapping::new()));
                if let serde_yml::Value::Mapping(schemas_map) = schemas {
                    for (k, v) in defs_map {
                        schemas_map.insert(k, v);
                    }
                }
            }
        }
    }
}

fn move_webhooks_to_paths(value: &mut serde_yml::Value) {
    let webhooks_key = serde_yml::Value::String("webhooks".to_string());
    let paths_key = serde_yml::Value::String("paths".to_string());
    let x_webhook_key = serde_yml::Value::String("x-webhook".to_string());

    if let serde_yml::Value::Mapping(map) = value {
        if let Some(serde_yml::Value::Mapping(webhooks_map)) = map.remove(&webhooks_key) {
            let paths = map
                .entry(paths_key)
                .or_insert_with(|| serde_yml::Value::Mapping(serde_yml::Mapping::new()));
            if let serde_yml::Value::Mapping(paths_map) = paths {
                for (path_key, mut path_item) in webhooks_map {
                    if let serde_yml::Value::Mapping(ref mut path_map) = path_item {
                        path_map.insert(x_webhook_key.clone(), serde_yml::Value::Bool(true));
                    }
                    paths_map.insert(path_key, path_item);
                }
            }
        }
    }
}

fn transform_schema(value: &mut serde_yml::Value) {
    match value {
        serde_yml::Value::Mapping(map) => {
            convert_type_array(map);
            convert_prefix_items(map);
            convert_bool_schemas(map);
            strip_nested_defs(map);
            for (_, v) in map.iter_mut() {
                transform_schema(v);
            }
        }
        serde_yml::Value::Sequence(seq) => {
            for item in seq {
                transform_schema(item);
            }
        }
        _ => {}
    }
}

fn convert_type_array(map: &mut serde_yml::Mapping) {
    let type_key = serde_yml::Value::String("type".to_string());
    let nullable_key = serde_yml::Value::String("nullable".to_string());

    let has_null = map
        .get(&type_key)
        .and_then(|v| v.as_sequence())
        .is_some_and(|seq| seq.iter().any(|v| v.as_str() == Some("null")));

    if !has_null {
        return;
    }

    let non_null: Option<String> =
        map.get(&type_key)
            .and_then(|v| v.as_sequence())
            .and_then(|seq| {
                seq.iter()
                    .find_map(|v| v.as_str().filter(|&s| s != "null").map(|s| s.to_string()))
            });

    map.insert(nullable_key, serde_yml::Value::Bool(true));

    if let Some(type_name) = non_null {
        map.insert(type_key, serde_yml::Value::String(type_name));
    } else {
        map.remove(&type_key);
    }
}

fn convert_prefix_items(map: &mut serde_yml::Mapping) {
    let prefix_key = serde_yml::Value::String("prefixItems".to_string());
    let items_key = serde_yml::Value::String("items".to_string());

    if let Some(serde_yml::Value::Sequence(seq)) = map.remove(&prefix_key) {
        if !seq.is_empty() && !map.contains_key(&items_key) {
            map.insert(items_key, seq.into_iter().next().unwrap());
        }
    }
}

fn convert_bool_schemas(map: &mut serde_yml::Mapping) {
    let schema_value_keys = ["schema", "items", "not", "contains", "if", "then", "else"];

    for key_str in &schema_value_keys {
        let key = serde_yml::Value::String(key_str.to_string());
        if let Some(v) = map.get_mut(&key) {
            if v.is_bool() {
                *v = bool_to_schema(v.as_bool().unwrap());
            }
        }
    }

    for key_str in &["oneOf", "allOf", "anyOf"] {
        let key = serde_yml::Value::String(key_str.to_string());
        if let Some(serde_yml::Value::Sequence(seq)) = map.get_mut(&key) {
            for item in seq.iter_mut() {
                if item.is_bool() {
                    *item = bool_to_schema(item.as_bool().unwrap());
                }
            }
        }
    }

    let props_key = serde_yml::Value::String("properties".to_string());
    if let Some(serde_yml::Value::Mapping(props)) = map.get_mut(&props_key) {
        for (_, prop_val) in props.iter_mut() {
            if prop_val.is_bool() {
                *prop_val = bool_to_schema(prop_val.as_bool().unwrap());
            }
        }
    }
}

fn bool_to_schema(b: bool) -> serde_yml::Value {
    if b {
        serde_yml::Value::Mapping(serde_yml::Mapping::new())
    } else {
        let mut map = serde_yml::Mapping::new();
        map.insert(
            serde_yml::Value::String("not".to_string()),
            serde_yml::Value::Mapping(serde_yml::Mapping::new()),
        );
        serde_yml::Value::Mapping(map)
    }
}

fn strip_nested_defs(map: &mut serde_yml::Mapping) {
    let defs_key = serde_yml::Value::String("$defs".to_string());
    map.remove(&defs_key);
}

pub fn load_contract_input(input: &str) -> Result<ApiContract> {
    load_contract_input_with_ref_root(input, None, None)
}

pub fn load_contract_input_with_ref_root(
    input: &str,
    ref_root: Option<PathBuf>,
    remote_headers: Option<&BTreeMap<String, String>>,
) -> Result<ApiContract> {
    if let Some(remote) = crate::remote::fetch(input, remote_headers)? {
        return load_remote_contract(remote);
    }

    load_contract_with_ref_root(Path::new(input), ref_root)
}

fn load_remote_contract(remote: crate::remote::RemoteOpenApi) -> Result<ApiContract> {
    load_contract_text(&remote.text, remote.is_json, "remote document", None)
        .map_err(|_| anyhow!("failed to parse remote OpenAPI document"))
}

fn validate_raw_openapi(raw: &str, is_json: bool) -> Result<()> {
    if is_json {
        let document: serde_json::Value =
            serde_json::from_str(raw).context("failed to parse OpenAPI JSON")?;
        validate_openapi_version(document.get("openapi").and_then(serde_json::Value::as_str))?;
        let Some(paths) = document.get("paths").and_then(serde_json::Value::as_object) else {
            return Ok(());
        };

        for path in paths.keys() {
            validate_raw_openapi_path(path)?;
        }
    } else {
        let document: serde_yml::Value =
            serde_yml::from_str(raw).context("failed to parse OpenAPI YAML")?;
        let openapi_key = serde_yml::Value::String("openapi".to_string());
        validate_openapi_version(
            document
                .as_mapping()
                .and_then(|document| document.get(&openapi_key))
                .and_then(serde_yml::Value::as_str),
        )?;

        let paths_key = serde_yml::Value::String("paths".to_string());
        let Some(paths) = document
            .as_mapping()
            .and_then(|document| document.get(&paths_key))
            .and_then(serde_yml::Value::as_mapping)
        else {
            return Ok(());
        };

        for path in paths.keys() {
            let path = path
                .as_str()
                .ok_or_else(|| anyhow!("OpenAPI path must be a string"))?;
            validate_raw_openapi_path(path)?;
        }
    }

    Ok(())
}

fn validate_openapi_version(version: Option<&str>) -> Result<()> {
    let Some(version) = version else {
        return Ok(());
    };

    if version == "3.0" || version.starts_with("3.0.") {
        return Ok(());
    }

    if version == "3.1" || version.starts_with("3.1.") {
        return Ok(());
    }

    Err(anyhow!(
        "unsupported OpenAPI version {version}; expected OpenAPI 3.0 or 3.1"
    ))
}

fn validate_raw_openapi_path(path: &str) -> Result<()> {
    if path.starts_with("x-") {
        return Ok(());
    }

    normalized_openapi_path(path)?;
    Ok(())
}

fn ensure_openapi_3(document: &OpenAPI) -> Result<()> {
    validate_openapi_version(Some(&document.openapi))
}

fn normalize(document: OpenAPI, ref_root: Option<PathBuf>) -> Result<ApiContract> {
    let mut contract = ApiContract::new();
    let schema_resolver = SchemaResolver::from_components(document.components.as_ref(), ref_root);
    let security_schemes = normalize_security_schemes(document.components.as_ref())?;
    let global_security = document.security.clone().unwrap_or_default();
    let root_servers = document.servers.clone();
    let context = OperationNormalizeContext {
        security_schemes: &security_schemes,
        schema_resolver: &schema_resolver,
        global_security: &global_security,
        root_servers: &root_servers,
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
            &item.servers,
            &item.parameters,
            item.get.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Post,
            &context,
            &item.servers,
            &item.parameters,
            item.post.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Put,
            &context,
            &item.servers,
            &item.parameters,
            item.put.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Patch,
            &context,
            &item.servers,
            &item.parameters,
            item.patch.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Delete,
            &context,
            &item.servers,
            &item.parameters,
            item.delete.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Options,
            &context,
            &item.servers,
            &item.parameters,
            item.options.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Head,
            &context,
            &item.servers,
            &item.parameters,
            item.head.as_ref(),
        )?;
        insert_operation(
            &mut contract,
            path,
            HttpMethod::Trace,
            &context,
            &item.servers,
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
    security_schemes: &'a BTreeMap<String, ResolvedAuthScheme>,
    schema_resolver: &'a SchemaResolver,
    global_security: &'a [SecurityRequirement],
    root_servers: &'a [Server],
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
    path_servers: &[Server],
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
    )?;
    let server_sources = if !operation.servers.is_empty() {
        &operation.servers
    } else if !path_servers.is_empty() {
        path_servers
    } else if !context.root_servers.is_empty() {
        context.root_servers
    } else {
        &[]
    };
    let servers = if server_sources.is_empty() {
        std::iter::once(identity::canonical_server_template("/")?).collect()
    } else {
        server_sources
            .iter()
            .map(|server| identity::canonical_server_template(&server.url))
            .collect::<Result<_>>()?
    };

    let parameters = normalize_parameters(
        context.schema_resolver,
        path_parameters,
        &operation.parameters,
    )?;
    let (canonical_path, placeholder_names) = identity::canonical_path_template(path)?;
    let parameters = canonicalize_path_parameters(parameters, &placeholder_names)?;

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

    let key = OperationKey {
        method,
        path: path.to_string(),
    };
    let identity = OperationIdentity {
        method,
        path: canonical_path,
    };
    if contract.operations.contains_key(&identity) {
        return Err(anyhow!(
            "ambiguous operation identity {} {}",
            identity.method.as_str(),
            identity.path
        ));
    }
    contract.operations.insert(
        identity,
        Operation {
            key,
            auth,
            servers: Some(servers),
            parameters,
            request_body,
            responses,
        },
    );

    Ok(())
}

fn canonicalize_path_parameters(
    parameters: BTreeMap<ParameterKey, Parameter>,
    placeholder_names: &[String],
) -> Result<BTreeMap<ParameterKey, Parameter>> {
    let mut canonical = BTreeMap::new();
    for (key, parameter) in parameters {
        let key = if key.location == ParameterLocation::Path {
            let slot = placeholder_names
                .iter()
                .position(|name| name == &parameter.name)
                .ok_or_else(|| {
                    anyhow!(
                        "path parameter {} is not bound to a path template placeholder",
                        parameter.name
                    )
                })?;
            ParameterKey {
                location: ParameterLocation::Path,
                name: format!("{{{slot}}}"),
            }
        } else {
            key
        };
        if canonical.insert(key, parameter).is_some() {
            return Err(anyhow!("duplicate path parameter binding"));
        }
    }
    for name in placeholder_names {
        if !canonical.iter().any(|(key, parameter)| {
            key.location == ParameterLocation::Path && parameter.name == *name
        }) {
            return Err(anyhow!(
                "path template placeholder {name} is not bound to a path parameter"
            ));
        }
    }
    Ok(canonical)
}

fn normalize_status_code(status: &StatusCode) -> String {
    match status {
        StatusCode::Code(_) | StatusCode::Range(_) => status.to_string(),
    }
}

#[derive(Clone, Debug)]
struct ResolvedAuthScheme {
    kind: AuthSchemeKind,
    identity: AuthIdentity,
}

fn normalize_security_schemes(
    components: Option<&Components>,
) -> Result<BTreeMap<String, ResolvedAuthScheme>> {
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
) -> Result<ResolvedAuthScheme> {
    match scheme {
        ReferenceOr::Item(scheme) => Ok(ResolvedAuthScheme {
            kind: auth_scheme_kind(scheme),
            identity: auth_scheme_identity(scheme)?,
        }),
        ReferenceOr::Reference { reference } => {
            resolve_security_scheme(reference, security_schemes, visiting)
        }
    }
}

fn resolve_security_scheme(
    reference: &str,
    security_schemes: &BTreeMap<String, ReferenceOr<OpenApiSecurityScheme>>,
    visiting: &mut BTreeSet<String>,
) -> Result<ResolvedAuthScheme> {
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

fn auth_scheme_identity(scheme: &OpenApiSecurityScheme) -> Result<AuthIdentity> {
    match scheme {
        OpenApiSecurityScheme::APIKey { location, name, .. } => {
            let location = match location {
                openapiv3::APIKeyLocation::Query => ParameterLocation::Query,
                openapiv3::APIKeyLocation::Header => ParameterLocation::Header,
                openapiv3::APIKeyLocation::Cookie => ParameterLocation::Cookie,
            };
            Ok(AuthIdentity::ApiKey {
                location,
                name: name.clone(),
            })
        }
        OpenApiSecurityScheme::HTTP { scheme, .. } => Ok(AuthIdentity::Http {
            scheme: scheme.to_ascii_lowercase(),
        }),
        OpenApiSecurityScheme::OAuth2 { flows, .. } => {
            let mut identities = BTreeSet::new();
            if let Some(flow) = &flows.implicit {
                identities.insert(OAuthFlowIdentity {
                    kind: OAuthFlowKind::Implicit,
                    authorization: Some(identity::canonical_auth_endpoint(
                        &flow.authorization_url,
                    )?),
                    token: None,
                    refresh: flow
                        .refresh_url
                        .as_deref()
                        .map(identity::canonical_auth_endpoint)
                        .transpose()?,
                });
            }
            if let Some(flow) = &flows.password {
                identities.insert(OAuthFlowIdentity {
                    kind: OAuthFlowKind::Password,
                    authorization: None,
                    token: Some(identity::canonical_auth_endpoint(&flow.token_url)?),
                    refresh: flow
                        .refresh_url
                        .as_deref()
                        .map(identity::canonical_auth_endpoint)
                        .transpose()?,
                });
            }
            if let Some(flow) = &flows.client_credentials {
                identities.insert(OAuthFlowIdentity {
                    kind: OAuthFlowKind::ClientCredentials,
                    authorization: None,
                    token: Some(identity::canonical_auth_endpoint(&flow.token_url)?),
                    refresh: flow
                        .refresh_url
                        .as_deref()
                        .map(identity::canonical_auth_endpoint)
                        .transpose()?,
                });
            }
            if let Some(flow) = &flows.authorization_code {
                identities.insert(OAuthFlowIdentity {
                    kind: OAuthFlowKind::AuthorizationCode,
                    authorization: Some(identity::canonical_auth_endpoint(
                        &flow.authorization_url,
                    )?),
                    token: Some(identity::canonical_auth_endpoint(&flow.token_url)?),
                    refresh: flow
                        .refresh_url
                        .as_deref()
                        .map(identity::canonical_auth_endpoint)
                        .transpose()?,
                });
            }
            Ok(AuthIdentity::OAuth2 { flows: identities })
        }
        OpenApiSecurityScheme::OpenIDConnect {
            open_id_connect_url,
            ..
        } => Ok(AuthIdentity::OpenIdConnect {
            discovery: identity::canonical_auth_endpoint(open_id_connect_url)?,
        }),
    }
}

fn normalize_auth_requirements(
    requirements: &[SecurityRequirement],
    security_schemes: &BTreeMap<String, ResolvedAuthScheme>,
) -> Result<BTreeMap<String, AuthRequirement>> {
    let mut auth = BTreeMap::new();

    if requirements
        .iter()
        .any(|requirement| requirement.is_empty())
    {
        return Ok(auth);
    }

    let mut identities = BTreeSet::new();

    for requirement in requirements {
        for (name, scopes) in requirement {
            let mut scopes = scopes.clone();
            scopes.sort();
            scopes.dedup();

            let scheme = security_schemes.get(name);
            let identity = scheme.map(|scheme| scheme.identity.clone());
            if let Some(identity) = &identity {
                if !matches!(identity, AuthIdentity::Unknown { .. })
                    && !identities.insert(identity.clone())
                {
                    return Err(anyhow!("duplicate authentication identity"));
                }
            }

            auth.insert(
                name.clone(),
                AuthRequirement {
                    name: name.clone(),
                    kind: scheme
                        .map(|scheme| scheme.kind)
                        .unwrap_or(AuthSchemeKind::Unknown),
                    identity: identity.or(Some(AuthIdentity::Unknown {
                        kind: AuthSchemeKind::Unknown,
                    })),
                    scopes,
                },
            );
        }
    }

    Ok(auth)
}

/// Upper bound on the number of concrete schema nodes `normalize_schema` may
/// construct while normalizing a single contract. Real-world specs with
/// densely-shared (but acyclic) component schemas can materialize into an
/// exponentially large fully-inlined tree even though the underlying
/// reference graph is modest in size; this budget turns that runaway
/// expansion into a prompt, deterministic error instead of an unbounded hang.
const MAX_SCHEMA_EXPANSIONS: usize = 3_000_000;

/// Counts the schema nodes reachable from `schema` (itself included). Used to
/// charge cache hits against the expansion budget in proportion to the size
/// of the (already fully-inlined) tree a clone would materialize, since a
/// cache hit is cheap to look up but not cheap to clone when the cached value
/// is a large, densely-shared subtree.
fn schema_node_count(schema: &Schema) -> usize {
    let mut count = 1;
    for property in schema.properties.values() {
        count += schema_node_count(&property.schema);
    }
    if let Some(items) = &schema.items {
        count += schema_node_count(items);
    }
    if let AdditionalProperties::Schema(inner) = &schema.additional_properties {
        count += schema_node_count(inner);
    }
    for branch in &schema.branches {
        count += schema_node_count(branch);
    }
    count
}

struct SchemaResolver {
    parameters: BTreeMap<String, ReferenceOr<OpenApiParameter>>,
    request_bodies: BTreeMap<String, ReferenceOr<OpenApiRequestBody>>,
    responses: BTreeMap<String, ReferenceOr<OpenApiResponse>>,
    schemas: BTreeMap<String, ReferenceOr<OpenApiSchema>>,
    ref_root: Option<PathBuf>,
    loaded_files: RefCell<BTreeMap<PathBuf, OpenAPI>>,
    normalized_schema_cache: RefCell<BTreeMap<String, Schema>>,
    normalized_schema_sizes: RefCell<BTreeMap<String, usize>>,
    expansion_count: Rc<RefCell<usize>>,
}

impl SchemaResolver {
    fn from_components(components: Option<&Components>, ref_root: Option<PathBuf>) -> Self {
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
            ref_root,
            loaded_files: RefCell::new(BTreeMap::new()),
            normalized_schema_cache: RefCell::new(BTreeMap::new()),
            normalized_schema_sizes: RefCell::new(BTreeMap::new()),
            expansion_count: Rc::new(RefCell::new(0)),
        }
    }

    fn record_expansion(&self, amount: usize) -> Result<()> {
        let mut count = self.expansion_count.borrow_mut();
        *count += amount;
        if *count > MAX_SCHEMA_EXPANSIONS {
            return Err(anyhow!(
                "schema expansion exceeded resolution budget of {MAX_SCHEMA_EXPANSIONS} nodes; \
                 the schema graph likely contains deeply/densely shared component schemas that \
                 expand exponentially when fully inlined"
            ));
        }
        Ok(())
    }

    fn resolve_parameter(
        &self,
        reference: &str,
        visiting: &mut BTreeSet<String>,
    ) -> Result<(ParameterKey, Parameter)> {
        let name = component_name(reference, "#/components/parameters/", "parameter")?;
        if !visiting.insert(name.clone()) {
            return Ok((
                ParameterKey {
                    location: ParameterLocation::Path,
                    name: format!("#/cycles/components/parameters/{name}"),
                },
                Parameter {
                    name: name.clone(),
                    required: false,
                    schema: Schema {
                        kind: SchemaKind::CycleRef,
                        nullable: false,
                        format: None,
                        enum_values: Vec::new(),
                        properties: BTreeMap::new(),
                        items: None,
                        additional_properties: AdditionalProperties::Forbidden,
                        branches: Vec::new(),
                        cycle_target: Some(format!("#/cycles/components/parameters/{name}")),
                    },
                },
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
            return Ok(RequestBody {
                required: Some(false),
                content: BTreeMap::from([(
                    "application/json".to_string(),
                    Schema {
                        kind: SchemaKind::CycleRef,
                        nullable: false,
                        format: None,
                        enum_values: Vec::new(),
                        properties: BTreeMap::new(),
                        items: None,
                        additional_properties: AdditionalProperties::Forbidden,
                        branches: Vec::new(),
                        cycle_target: Some(format!("#/cycles/components/requestBodies/{name}")),
                    },
                )]),
            });
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
            return Ok(Response {
                content: BTreeMap::from([(
                    "application/json".to_string(),
                    Schema {
                        kind: SchemaKind::CycleRef,
                        nullable: false,
                        format: None,
                        enum_values: Vec::new(),
                        properties: BTreeMap::new(),
                        items: None,
                        additional_properties: AdditionalProperties::Forbidden,
                        branches: Vec::new(),
                        cycle_target: Some(format!("#/cycles/components/responses/{name}")),
                    },
                )]),
            });
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
        if let Some(hash_pos) = reference.find('#') {
            if hash_pos > 0 {
                return self.resolve_external(reference, hash_pos, visiting);
            }
        }

        let name = component_name(reference, "#/components/schemas/", "schema")?;
        if !visiting.insert(name.clone()) {
            return Ok(Schema {
                kind: SchemaKind::CycleRef,
                nullable: false,
                format: None,
                enum_values: Vec::new(),
                properties: BTreeMap::new(),
                items: None,
                additional_properties: AdditionalProperties::Forbidden,
                branches: Vec::new(),
                cycle_target: Some(format!("#/cycles/components/schemas/{name}")),
            });
        }

        if let Some(cached) = self.normalized_schema_cache.borrow().get(&name).cloned() {
            visiting.remove(&name);
            let size = *self
                .normalized_schema_sizes
                .borrow()
                .get(&name)
                .unwrap_or(&1);
            self.record_expansion(size)?;
            return Ok(cached);
        }

        let schema = self
            .schemas
            .get(&name)
            .ok_or_else(|| anyhow!("schema reference not found: {reference}"))?;
        let normalized = normalize_schema_ref(schema, self, visiting);
        visiting.remove(&name);
        if let Ok(ref result) = normalized {
            if !matches!(result.kind, SchemaKind::CycleRef) {
                self.normalized_schema_sizes
                    .borrow_mut()
                    .insert(name.clone(), schema_node_count(result));
                self.normalized_schema_cache
                    .borrow_mut()
                    .insert(name, result.clone());
            }
        }
        normalized
    }

    fn resolve_external(
        &self,
        reference: &str,
        hash_pos: usize,
        visiting: &mut BTreeSet<String>,
    ) -> Result<Schema> {
        let file_part = &reference[..hash_pos];
        let pointer = &reference[hash_pos..];

        if file_part.starts_with("https://") || file_part.starts_with("http://") {
            return Err(anyhow!(
                "remote references are not yet supported: {reference}"
            ));
        }

        let ref_root = self.ref_root.as_deref().unwrap_or_else(|| Path::new("."));

        let canonical_root = ref_root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize resolution root {:?}", ref_root))?;

        let resolved = safe_resolve_ref(&canonical_root, file_part)?;

        let canonical = resolved
            .canonicalize()
            .with_context(|| format!("external file not found: {:?}", resolved))?;

        if !canonical.starts_with(&canonical_root) {
            return Err(anyhow!(
                "external reference resolves outside the resolution root"
            ));
        }

        {
            let mut loaded = self.loaded_files.borrow_mut();
            if !loaded.contains_key(&canonical) {
                let doc = parse_external_file(&canonical)?;
                loaded.insert(canonical.clone(), doc);
            }
        }

        let loaded = self.loaded_files.borrow();
        let doc = loaded.get(&canonical).unwrap();

        let name = component_name(pointer, "#/components/schemas/", "schema")?;

        let visit_key = format!("{}:{}", canonical.display(), name);
        if !visiting.insert(visit_key.clone()) {
            return Ok(Schema {
                kind: SchemaKind::CycleRef,
                nullable: false,
                format: None,
                enum_values: Vec::new(),
                properties: BTreeMap::new(),
                items: None,
                additional_properties: AdditionalProperties::Forbidden,
                branches: Vec::new(),
                cycle_target: Some(format!(
                    "#/cycles/external/{}#/components/schemas/{name}",
                    canonical.display()
                )),
            });
        }

        if let Some(cached) = self
            .normalized_schema_cache
            .borrow()
            .get(&visit_key)
            .cloned()
        {
            visiting.remove(&visit_key);
            let size = *self
                .normalized_schema_sizes
                .borrow()
                .get(&visit_key)
                .unwrap_or(&1);
            self.record_expansion(size)?;
            return Ok(cached);
        }

        let schema_ref = doc
            .components
            .as_ref()
            .and_then(|c| c.schemas.get(&name))
            .ok_or_else(|| anyhow!("schema {} not found in external file {:?}", name, canonical))?;

        let mut external_resolver =
            SchemaResolver::from_components(doc.components.as_ref(), Some(canonical_root.clone()));

        *external_resolver.loaded_files.borrow_mut() = loaded.clone();
        *external_resolver.normalized_schema_cache.borrow_mut() =
            self.normalized_schema_cache.borrow().clone();
        *external_resolver.normalized_schema_sizes.borrow_mut() =
            self.normalized_schema_sizes.borrow().clone();
        external_resolver.expansion_count = self.expansion_count.clone();

        let result = normalize_schema_ref(schema_ref, &external_resolver, visiting);
        visiting.remove(&visit_key);
        if let Ok(ref schema) = result {
            if !matches!(schema.kind, SchemaKind::CycleRef) {
                self.normalized_schema_sizes
                    .borrow_mut()
                    .insert(visit_key.clone(), schema_node_count(schema));
                self.normalized_schema_cache
                    .borrow_mut()
                    .insert(visit_key, schema.clone());
            }
        }
        result
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

fn parse_external_file(path: &Path) -> Result<OpenAPI> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read external file {}", path.display()))?;
    let is_json = path.extension().and_then(|v| v.to_str()) == Some("json");
    if is_json {
        serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse external JSON {}", path.display()))
            .or_else(|_| parse_external_components_json(&raw, path))
    } else {
        tolerant_openapi_yaml(raw.as_bytes())
            .with_context(|| format!("failed to parse external YAML {}", path.display()))
            .or_else(|_| parse_external_components_yaml(raw.as_bytes(), path))
    }
}

fn parse_external_components_json(raw: &str, path: &Path) -> Result<OpenAPI> {
    let components: openapiv3::Components = serde_json::from_str(raw).with_context(|| {
        format!(
            "failed to parse external JSON components {}",
            path.display()
        )
    })?;
    Ok(OpenAPI {
        openapi: "3.0.3".to_string(),
        info: openapiv3::Info {
            title: "external-fragment".to_string(),
            version: "0.0.0".to_string(),
            ..Default::default()
        },
        paths: openapiv3::Paths::default(),
        components: Some(components),
        ..Default::default()
    })
}

fn parse_external_components_yaml(bytes: &[u8], path: &Path) -> Result<OpenAPI> {
    let mut value: serde_yml::Value =
        serde_yml::from_slice(bytes).context("failed to parse external YAML")?;
    strip_deep(&mut value, "tags");
    strip_deep(&mut value, "externalDocs");
    strip_deep(&mut value, "examples");
    strip_deep(&mut value, "callbacks");
    strip_deep_by_prefix(&mut value, "x-");
    if let Some(components) = value.get_mut("components") {
        let cleaned = serde_yml::to_string(components).context("failed to serialize components")?;
        let components: openapiv3::Components =
            serde_yml::from_str(&cleaned).with_context(|| {
                format!(
                    "failed to parse external YAML components {}",
                    path.display()
                )
            })?;
        Ok(OpenAPI {
            openapi: "3.0.3".to_string(),
            info: openapiv3::Info {
                title: "external-fragment".to_string(),
                version: "0.0.0".to_string(),
                ..Default::default()
            },
            paths: openapiv3::Paths::default(),
            components: Some(components),
            ..Default::default()
        })
    } else {
        Err(anyhow::anyhow!(
            "external file {} contains no components",
            path.display()
        ))
    }
}

fn safe_resolve_ref(root: &Path, file_part: &str) -> Result<PathBuf> {
    let mut components: Vec<&std::ffi::OsStr> = Vec::new();
    for component in Path::new(file_part).components() {
        match component {
            std::path::Component::ParentDir => {
                if components.pop().is_none() {
                    return Err(anyhow!(
                        "external reference {:?} escapes the resolution root",
                        file_part
                    ));
                }
            }
            std::path::Component::Normal(seg) => components.push(seg),
            std::path::Component::CurDir => {}
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                return Err(anyhow!(
                    "external reference {:?} must be a relative path",
                    file_part
                ));
            }
        }
    }

    let mut resolved = root.to_path_buf();
    for comp in components {
        resolved.push(comp);
    }

    Ok(resolved)
}

fn normalize_parameters(
    schema_resolver: &SchemaResolver,
    path_parameters: &[ReferenceOr<OpenApiParameter>],
    operation_parameters: &[ReferenceOr<OpenApiParameter>],
) -> Result<BTreeMap<ParameterKey, Parameter>> {
    let mut parameters = BTreeMap::new();
    let mut path_keys = BTreeSet::new();

    for parameter in path_parameters {
        let (key, parameter) =
            normalize_parameter_ref(parameter, schema_resolver, &mut BTreeSet::new())?;
        if !path_keys.insert(key.clone()) {
            return Err(anyhow!(
                "duplicate parameter {}:{}",
                key.location.as_str(),
                key.name
            ));
        }
        parameters.insert(key, parameter);
    }

    let mut operation_keys = BTreeSet::new();
    for parameter in operation_parameters {
        let (key, parameter) =
            normalize_parameter_ref(parameter, schema_resolver, &mut BTreeSet::new())?;
        if !operation_keys.insert(key.clone()) {
            return Err(anyhow!(
                "duplicate parameter {}:{}",
                key.location.as_str(),
                key.name
            ));
        }
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
    if data.name.is_empty() || data.name.chars().any(char::is_control) {
        return Err(anyhow!(
            "parameter name contains invalid characters: {:?}",
            data.name
        ));
    }
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
            identity::canonical_media_type(content_type)?,
            normalize_media_type(media_type, schema_resolver)?,
        );
    }

    Ok(RequestBody {
        required: Some(request_body.required),
        content,
    })
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
            identity::canonical_media_type(content_type)?,
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
    schema_resolver.record_expansion(1)?;

    let mut normalized = unknown_schema();
    normalized.nullable = schema.schema_data.nullable;

    match &schema.schema_kind {
        OpenApiSchemaKind::Type(Type::Object(object)) => {
            normalized.kind = SchemaKind::Object;
            normalized.additional_properties = match &object.additional_properties {
                None | Some(OpenApiAdditionalProperties::Any(true)) => AdditionalProperties::Any,
                Some(OpenApiAdditionalProperties::Any(false)) => AdditionalProperties::Forbidden,
                Some(OpenApiAdditionalProperties::Schema(schema)) => {
                    AdditionalProperties::Schema(Box::new(normalize_schema_ref(
                        schema.as_ref(),
                        schema_resolver,
                        visiting,
                    )?))
                }
            };
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
                normalized.items = Some(Box::new(normalize_boxed_schema_ref(
                    items,
                    schema_resolver,
                    visiting,
                )?));
            }
        }
        OpenApiSchemaKind::OneOf { one_of } => {
            normalized.kind = SchemaKind::OneOf;
            normalized.branches =
                normalize_composed_schema_refs(one_of, schema_resolver, visiting)?;
        }
        OpenApiSchemaKind::AllOf { all_of } => {
            let mut merged = merge_all_of(normalize_composed_schema_refs(
                all_of,
                schema_resolver,
                visiting,
            )?)?;
            merged.nullable &= normalized.nullable;
            normalized = merged;
        }
        OpenApiSchemaKind::AnyOf { any_of } => {
            normalized.kind = SchemaKind::AnyOf;
            normalized.branches =
                normalize_composed_schema_refs(any_of, schema_resolver, visiting)?;
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
        OpenApiSchemaKind::Type(Type::Boolean(boolean)) => {
            normalized.kind = SchemaKind::Boolean;
            normalized.enum_values = boolean
                .enumeration
                .iter()
                .flatten()
                .map(|value| value.to_string())
                .collect();
        }
        _ => {
            normalized.kind = SchemaKind::Unknown;
        }
    }

    normalized.enum_values.sort();
    normalized.enum_values.dedup();

    Ok(normalized)
}

fn normalize_composed_schema_refs(
    schemas: &[ReferenceOr<OpenApiSchema>],
    schema_resolver: &SchemaResolver,
    visiting: &mut BTreeSet<String>,
) -> Result<Vec<Schema>> {
    let mut branches = schemas
        .iter()
        .map(|schema| normalize_schema_ref(schema, schema_resolver, visiting))
        .collect::<Result<Vec<_>>>()?;
    branches.sort_by_key(Schema::structural_key);
    branches.dedup_by(|left, right| left.structural_key() == right.structural_key());
    Ok(branches)
}

pub fn merge_all_of(mut schemas: Vec<Schema>) -> Result<Schema> {
    let Some(mut merged) = schemas.pop() else {
        return Ok(unknown_schema());
    };
    for schema in schemas {
        merged = intersect_schema(merged, schema)?;
    }
    Ok(merged)
}

fn intersect_schema(mut left: Schema, right: Schema) -> Result<Schema> {
    let left_was_unknown = matches!(left.kind, SchemaKind::Unknown);
    let right_was_unknown = matches!(right.kind, SchemaKind::Unknown);
    left.kind = match (&left.kind, &right.kind) {
        (SchemaKind::Unknown, kind) => kind.clone(),
        (kind, SchemaKind::Unknown) => kind.clone(),
        (left_kind, right_kind) if left_kind == right_kind => left_kind.clone(),
        _ => return Err(anyhow!("allOf contains incompatible schema kinds")),
    };
    left.nullable &= right.nullable;
    left.format = match (left.format.take(), right.format) {
        (None, format) | (format, None) => format,
        (Some(left_format), Some(right_format)) if left_format == right_format => Some(left_format),
        _ => return Err(anyhow!("allOf contains incompatible schema formats")),
    };
    left.enum_values = intersect_enums(left.enum_values, right.enum_values)?;
    for (name, right_property) in right.properties {
        if let Some(left_property) = left.properties.get_mut(&name) {
            left_property.required |= right_property.required;
            *left_property.schema =
                intersect_schema(*left_property.schema.clone(), *right_property.schema)?;
        } else {
            left.properties.insert(name, right_property);
        }
    }
    left.items = match (left.items.take(), right.items) {
        (Some(left_items), Some(right_items)) => {
            Some(Box::new(intersect_schema(*left_items, *right_items)?))
        }
        (Some(items), None) | (None, Some(items)) => Some(items),
        (None, None) => None,
    };
    left.additional_properties = match (left_was_unknown, right_was_unknown) {
        (true, false) => right.additional_properties,
        (false, true) => left.additional_properties,
        _ => intersect_additional_properties(
            left.additional_properties,
            right.additional_properties,
        )?,
    };
    match (left_was_unknown, right_was_unknown) {
        (true, false) => left.branches = right.branches,
        (false, true) => {}
        _ if !left.branches.is_empty() || !right.branches.is_empty() => {
            return Err(anyhow!("allOf branches must be merged before intersection"));
        }
        _ => {}
    }
    Ok(left)
}
fn intersect_enums(left: Vec<String>, right: Vec<String>) -> Result<Vec<String>> {
    if left.is_empty() {
        return Ok(right);
    }
    if right.is_empty() {
        return Ok(left);
    }
    let right = right.into_iter().collect::<BTreeSet<_>>();
    let values = left
        .into_iter()
        .filter(|value| right.contains(value))
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Err(anyhow!("allOf enum intersection is empty"));
    }
    Ok(values)
}
fn intersect_additional_properties(
    left: AdditionalProperties,
    right: AdditionalProperties,
) -> Result<AdditionalProperties> {
    use AdditionalProperties::{Any, Forbidden, Schema, Unknown};
    match (left, right) {
        (Unknown, policy) | (policy, Unknown) => Ok(policy),
        (Forbidden, _) | (_, Forbidden) => Ok(Forbidden),
        (Any, policy) | (policy, Any) => Ok(policy),
        (Schema(left), Schema(right)) => Ok(Schema(Box::new(intersect_schema(*left, *right)?))),
    }
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
        items: None,
        additional_properties: AdditionalProperties::Forbidden,
        branches: Vec::new(),
        cycle_target: None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use crate::contract::{AdditionalProperties, HttpMethod, Property, Schema, SchemaKind};
    use crate::remote::RemoteOpenApi;

    use super::{load_contract, load_remote_contract, merge_all_of};

    fn schema(kind: SchemaKind) -> Schema {
        Schema {
            kind,
            nullable: false,
            format: None,
            enum_values: Vec::new(),
            properties: BTreeMap::new(),
            items: None,
            additional_properties: AdditionalProperties::Any,
            branches: Vec::new(),
            cycle_target: None,
        }
    }

    #[test]
    fn allof_intersection_preserves_the_narrowest_recursive_constraints() {
        let mut left = schema(SchemaKind::Object);
        left.nullable = true;
        left.properties.insert(
            "items".to_string(),
            Property {
                required: false,
                schema: Box::new(schema(SchemaKind::Unknown)),
            },
        );
        left.additional_properties =
            AdditionalProperties::Schema(Box::new(schema(SchemaKind::Unknown)));
        let mut right = schema(SchemaKind::Object);
        right.properties.insert(
            "items".to_string(),
            Property {
                required: true,
                schema: Box::new(Schema {
                    format: Some("uuid".to_string()),
                    enum_values: vec!["a".to_string(), "b".to_string()],
                    cycle_target: None,
                    ..schema(SchemaKind::String)
                }),
            },
        );
        right.additional_properties = AdditionalProperties::Schema(Box::new(Schema {
            enum_values: vec!["a".to_string()],
            cycle_target: None,
            ..schema(SchemaKind::String)
        }));

        let merged = merge_all_of(vec![left, right]).expect("compatible allOf should merge");

        assert!(!merged.nullable);
        let items = merged.properties.get("items").expect("items should merge");
        assert!(items.required);
        assert_eq!(items.schema.kind, SchemaKind::String);
        assert_eq!(items.schema.format.as_deref(), Some("uuid"));
        match merged.additional_properties {
            AdditionalProperties::Schema(value) => assert_eq!(value.enum_values, ["a"]),
            _ => panic!("additionalProperties should stay schema constrained"),
        }
    }

    #[test]
    fn allof_intersection_rejects_incompatible_constraints() {
        let error = merge_all_of(vec![
            schema(SchemaKind::String),
            schema(SchemaKind::Integer),
        ])
        .expect_err("different known kinds cannot intersect");

        assert!(error.to_string().contains("incompatible schema kinds"));
    }

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

    #[test]
    fn normalizes_array_items_as_first_class_schema() {
        let contract = load_contract(Path::new(
            "testdata/openapi/phase2_d10_array_items_old.yaml",
        ))
        .expect("fixture should parse");
        let schema = &contract
            .operations
            .values()
            .find(|operation| {
                operation.key.path == "/users" && operation.key.method == HttpMethod::Get
            })
            .expect("GET /users should exist")
            .responses["200"]
            .content["application/json"];

        assert!(
            schema.items.is_some(),
            "array items should be normalized directly"
        );
        assert!(!schema.properties.contains_key("items"));
    }

    #[test]
    fn remote_parse_errors_are_sanitized() {
        let raw_content = "not: [valid\x1b remote OpenAPI";
        let error = load_remote_contract(RemoteOpenApi {
            text: raw_content.to_string(),
            is_json: false,
        })
        .expect_err("invalid remote document should fail");

        assert_eq!(error.to_string(), "failed to parse remote OpenAPI document");
        assert!(!error.to_string().contains(raw_content));
        assert!(!error.to_string().chars().any(char::is_control));
    }
}
