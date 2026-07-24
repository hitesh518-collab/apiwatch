use std::collections::BTreeMap;

use serde::Serialize;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
    pub content: BTreeMap<String, Schema>,
}

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
