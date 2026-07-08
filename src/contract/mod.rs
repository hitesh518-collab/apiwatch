use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub responses: BTreeMap<String, Response>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub content: BTreeMap<String, Schema>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub kind: SchemaKind,
    pub nullable: bool,
    pub format: Option<String>,
    pub enum_values: Vec<String>,
    pub properties: BTreeMap<String, Property>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaKind {
    Object,
    Array,
    String,
    Integer,
    Number,
    Boolean,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub required: bool,
    pub schema: Box<Schema>,
}
