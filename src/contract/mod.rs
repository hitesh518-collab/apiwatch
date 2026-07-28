use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ApiContract {
    pub operations: BTreeMap<OperationKey, Operation>,
}

impl ApiContract {
    pub fn new() -> Self {
        Self {
            operations: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperationKey {
    pub method: HttpMethod,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Options,
    Head,
    Trace,
}

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
            Self::Head => "HEAD",
            Self::Trace => "TRACE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Operation {
    pub auth: BTreeMap<String, AuthRequirement>,
    pub servers: Option<BTreeSet<ServerTemplate>>,
    pub parameters: BTreeMap<ParameterKey, Parameter>,
    pub request_body: Option<RequestBody>,
    pub responses: BTreeMap<String, Response>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthRequirement {
    pub name: String,
    pub kind: AuthSchemeKind,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthSchemeKind {
    ApiKey,
    Basic,
    Bearer,
    OAuth2,
    OpenIdConnect,
    Http,
    Unknown,
}

impl AuthSchemeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ApiKey => "apiKey",
            Self::Basic => "basic",
            Self::Bearer => "bearer",
            Self::OAuth2 => "oauth2",
            Self::OpenIdConnect => "openIdConnect",
            Self::Http => "http",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParameterKey {
    pub location: ParameterLocation,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

impl ParameterLocation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Query => "query",
            Self::Header => "header",
            Self::Cookie => "cookie",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Parameter {
    pub name: String,
    pub required: bool,
    pub schema: Schema,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequestBody {
    pub required: Option<bool>,
    pub content: BTreeMap<String, Schema>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ServerTemplate(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Response {
    pub content: BTreeMap<String, Schema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Schema {
    pub kind: SchemaKind,
    pub nullable: bool,
    pub format: Option<String>,
    pub enum_values: Vec<String>,
    pub properties: BTreeMap<String, Property>,
    pub additional_properties: AdditionalProperties,
    pub branches: Vec<Schema>,
}

impl Schema {
    pub fn structural_key(&self) -> String {
        let mut encoded = String::new();
        self.encode_structural(&mut encoded);
        format!("sha256:{:x}", Sha256::digest(encoded.as_bytes()))
    }

    pub fn shape_key(&self) -> String {
        let mut encoded = String::new();
        self.encode_shape(&mut encoded);
        format!("sha256:{:x}", Sha256::digest(encoded.as_bytes()))
    }

    fn encode_structural(&self, encoded: &mut String) {
        encode_field(encoded, schema_kind_tag(&self.kind));
        encode_field(encoded, if self.nullable { "1" } else { "0" });
        encode_option(encoded, self.format.as_deref());
        encode_values(encoded, &self.enum_values);
        for (name, property) in &self.properties {
            encode_field(encoded, name);
            encode_field(encoded, if property.required { "1" } else { "0" });
            property.schema.encode_structural(encoded);
        }
        encode_additional_properties(encoded, &self.additional_properties);
        for branch in &self.branches {
            branch.encode_structural(encoded);
        }
    }

    fn encode_shape(&self, encoded: &mut String) {
        encode_field(encoded, schema_kind_tag(&self.kind));
        self.encode_topology(encoded);
    }

    fn encode_topology(&self, encoded: &mut String) {
        for (name, property) in &self.properties {
            encode_field(encoded, name);
            property.schema.encode_topology(encoded);
        }
        for branch in &self.branches {
            branch.encode_topology(encoded);
        }
    }
}

fn encode_field(encoded: &mut String, value: &str) {
    encoded.push_str(&value.len().to_string());
    encoded.push(':');
    encoded.push_str(value);
    encoded.push(';');
}
fn encode_option(encoded: &mut String, value: Option<&str>) {
    encode_field(encoded, value.unwrap_or(""));
}
fn encode_values(encoded: &mut String, values: &[String]) {
    encode_field(encoded, &values.len().to_string());
    for value in values {
        encode_field(encoded, value);
    }
}
fn encode_additional_properties(encoded: &mut String, policy: &AdditionalProperties) {
    match policy {
        AdditionalProperties::Schema(schema) => {
            encode_field(encoded, "schema");
            schema.encode_structural(encoded);
        }
        policy => encode_field(encoded, additional_properties_tag(policy)),
    }
}
fn additional_properties_tag(policy: &AdditionalProperties) -> &'static str {
    match policy {
        AdditionalProperties::Unknown => "unknown",
        AdditionalProperties::Forbidden => "forbidden",
        AdditionalProperties::Any => "any",
        AdditionalProperties::Schema(_) => "schema",
    }
}
fn schema_kind_tag(kind: &SchemaKind) -> &'static str {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum AdditionalProperties {
    Unknown,
    Forbidden,
    Any,
    Schema(Box<Schema>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SchemaKind {
    Object,
    Array,
    OneOf,
    AllOf,
    AnyOf,
    String,
    Integer,
    Number,
    Boolean,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Property {
    pub required: bool,
    pub schema: Box<Schema>,
}

impl Serialize for OperationKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{} {}", self.method.as_str(), self.path))
    }
}

impl Serialize for ParameterKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}:{}", self.location.as_str(), self.name))
    }
}
