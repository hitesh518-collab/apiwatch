use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use openapiv3::{
    IntegerFormat, MediaType, NumberFormat, OpenAPI, Operation as OpenApiOperation, PathItem,
    ReferenceOr, Response as OpenApiResponse, Schema as OpenApiSchema,
    SchemaKind as OpenApiSchemaKind, StatusCode, StringFormat, Type, VariantOrUnknownOrEmpty,
};

use crate::contract::{
    ApiContract, HttpMethod, Operation, OperationKey, Property, Response, Schema, SchemaKind,
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

    normalize(document)
}

fn normalize(document: OpenAPI) -> Result<ApiContract> {
    let mut contract = ApiContract::new();

    for (path, item) in document.paths.paths {
        let item = resolve_path_item(item)?;
        insert_operation(&mut contract, &path, HttpMethod::Get, item.get.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Post, item.post.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Put, item.put.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Patch, item.patch.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Delete, item.delete.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Options, item.options.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Head, item.head.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Trace, item.trace.as_ref())?;
    }

    Ok(contract)
}

fn resolve_path_item(item: ReferenceOr<PathItem>) -> Result<PathItem> {
    match item {
        ReferenceOr::Item(item) => Ok(item),
        ReferenceOr::Reference { reference } => Err(anyhow!(
            "path item references are not supported yet: {reference}"
        )),
    }
}

fn insert_operation(
    contract: &mut ApiContract,
    path: &str,
    method: HttpMethod,
    operation: Option<&OpenApiOperation>,
) -> Result<()> {
    let Some(operation) = operation else {
        return Ok(());
    };

    let mut responses = BTreeMap::new();
    for (status, response) in &operation.responses.responses {
        let status = normalize_status_code(status);
        let response = normalize_response(response)?;
        responses.insert(status, response);
    }

    contract.operations.insert(
        OperationKey {
            method,
            path: path.to_string(),
        },
        Operation { responses },
    );

    Ok(())
}

fn normalize_status_code(status: &StatusCode) -> String {
    match status {
        StatusCode::Code(_) | StatusCode::Range(_) => status.to_string(),
    }
}

fn normalize_response(response: &ReferenceOr<OpenApiResponse>) -> Result<Response> {
    let response = match response {
        ReferenceOr::Item(response) => response,
        ReferenceOr::Reference { reference } => {
            return Err(anyhow!("response references are not supported yet: {reference}"));
        }
    };

    let mut content = BTreeMap::new();
    for (content_type, media_type) in &response.content {
        content.insert(content_type.clone(), normalize_media_type(media_type)?);
    }

    Ok(Response { content })
}

fn normalize_media_type(media_type: &MediaType) -> Result<Schema> {
    match &media_type.schema {
        Some(schema) => normalize_schema_ref(schema),
        None => Ok(unknown_schema()),
    }
}

fn normalize_schema_ref(schema: &ReferenceOr<OpenApiSchema>) -> Result<Schema> {
    let schema = match schema {
        ReferenceOr::Item(schema) => schema,
        ReferenceOr::Reference { reference } => {
            return Err(anyhow!("schema references are not supported yet: {reference}"));
        }
    };

    normalize_schema(schema)
}

fn normalize_boxed_schema_ref(schema: &ReferenceOr<Box<OpenApiSchema>>) -> Result<Schema> {
    let schema = match schema {
        ReferenceOr::Item(schema) => schema.as_ref(),
        ReferenceOr::Reference { reference } => {
            return Err(anyhow!("schema references are not supported yet: {reference}"));
        }
    };

    normalize_schema(schema)
}

fn normalize_schema(schema: &OpenApiSchema) -> Result<Schema> {
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
                    let schema = normalize_boxed_schema_ref(schema)?;
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
                        schema: Box::new(normalize_boxed_schema_ref(items)?),
                    },
                );
            }
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

        let operation = contract.operations.get(key).expect("operation should exist");
        assert!(operation.responses.contains_key("200"));
    }
}
