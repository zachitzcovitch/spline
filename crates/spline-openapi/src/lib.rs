//! Deterministic OpenAPI 3.1.2 export from sealed Spline HTTP descriptions.
//!
//! The exporter consumes directional Spline schemas and HTTP plans. It has no
//! Axum/reqwest dependency; deployment metadata is explicit export configuration.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;
use serde_json::{json, Value};
use spline_contract::ResourceDescription;
use spline_http_contract::{ApplicationDescription, ContractDescription, HttpPlan};
use spline_schema::{DefinitionId, Graph};

mod operation;
mod schema;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Configuration(String),
    ComponentCollision(String),
    InvalidComponentName(String),
    Schema(spline_schema::Error),
    UnguardedRecursion(Vec<DefinitionId>),
}
impl From<spline_schema::Error> for Error {
    fn from(error: spline_schema::Error) -> Self {
        Self::Schema(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(message) => write!(f, "invalid OpenAPI configuration: {message}"),
            Self::ComponentCollision(name) => write!(f, "OpenAPI component collision: {name}; use OpenApi::name for an explicit directional override"),
            Self::InvalidComponentName(name) => write!(f, "invalid OpenAPI component name: {name}"),
            Self::Schema(error) => error.fmt(f),
            Self::UnguardedRecursion(cycle) => write!(f, "unguarded schema reference cycle: {cycle:?}"),
        }
    }
}
impl std::error::Error for Error {}

/// Inspectable, editable OpenAPI JSON with deterministic serialization helpers.
/// Validate the final document independently after making manual edits.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Document(Value);

impl Document {
    pub fn as_json(&self) -> &Value {
        &self.0
    }
    pub fn as_json_mut(&mut self) -> &mut Value {
        &mut self.0
    }
    pub fn into_json(self) -> Value {
        self.0
    }
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(&canonical(self.0.clone()))
    }
    /// Pretty JSON with a trailing newline, suitable for checked-in artifacts.
    pub fn to_pretty_json(&self) -> serde_json::Result<String> {
        let mut source = serde_json::to_string_pretty(&canonical(self.0.clone()))?;
        source.push('\n');
        Ok(source)
    }
}

fn canonical(mut value: Value) -> Value {
    match &mut value {
        Value::Object(object) => {
            object.sort_keys();
            for child in object.values_mut() {
                *child = canonical(child.take());
            }
        }
        Value::Array(array) => {
            for child in array {
                *child = canonical(child.take());
            }
        }
        _ => {}
    }
    value
}

/// OpenAPI document configuration. Paths, DTOs, and responses come from the binding.
#[derive(Clone, Debug)]
pub struct OpenApi {
    title: String,
    version: String,
    description: Option<String>,
    servers: Vec<String>,
    bearer_auth: Option<String>,
    names: BTreeMap<DefinitionId, String>,
}

impl OpenApi {
    pub fn new(title: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            version: version.into(),
            description: None,
            servers: Vec::new(),
            bearer_auth: None,
            names: BTreeMap::new(),
        }
    }
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
    /// Add a static HTTP(S) or root-relative server URL, including any mount prefix.
    pub fn server(mut self, url: impl Into<String>) -> Self {
        self.servers.push(url.into());
        self
    }
    /// Define an HTTP bearer scheme and require it globally. This is documentation
    /// configuration; the application remains responsible for authentication.
    pub fn bearer_auth(mut self, name: impl Into<String>) -> Self {
        self.bearer_auth = Some(name.into());
        self
    }
    /// Override one directional component name, independently of logical identity.
    pub fn name(mut self, id: DefinitionId, name: impl Into<String>) -> Self {
        self.names.insert(id, name.into());
        self
    }
    pub fn export_contract(&self, binding: &ContractDescription) -> Result<Document> {
        self.export(
            binding.contract().schemas(),
            &[(
                binding.contract().namespace(),
                binding.contract().resource(),
            )],
            binding.plans(),
            binding.contract().documentation(),
        )
    }
    pub fn export_application(&self, binding: &ApplicationDescription) -> Result<Document> {
        let resources: Vec<_> = binding
            .application()
            .resources()
            .iter()
            .map(|(group, resource)| (group.as_str(), resource))
            .collect();
        self.export(
            binding.application().schemas(),
            &resources,
            binding.plans(),
            binding.application().documentation(),
        )
    }

    fn export(
        &self,
        graph: &Graph,
        resources: &[(&str, &ResourceDescription)],
        plans: &BTreeMap<String, HttpPlan>,
        documentation: &str,
    ) -> Result<Document> {
        self.validate()?;
        let operations: Vec<_> = resources
            .iter()
            .flat_map(|(_, resource)| resource.operations().values())
            .collect();
        let schemas = schema::Schemas::new(graph, &operations, plans, &self.names)?;
        let mut components = schemas.components()?;
        components.insert(
            operation::FRAMEWORK_SCHEMA.into(),
            schemas.render(&spline_http_contract::framework_error_schema())?,
        );
        let mut paths = serde_json::Map::new();
        let mut tags = Vec::new();
        for (group, resource) in resources {
            let mut tag = json!({"name":group});
            if !resource.documentation().is_empty() {
                tag["description"] = resource.documentation().into();
            }
            tags.push(tag);
            for op in resource.operations().values() {
                let plan = &plans[&op.id];
                let path = paths
                    .entry(plan.binding.path.clone())
                    .or_insert_with(|| json!({}));
                path[operation::method(plan.binding.method)] =
                    operation::render(op, plan, group, &schemas)?;
            }
        }
        let mut info = json!({"title":self.title,"version":self.version});
        let description = self.description.as_deref().unwrap_or(documentation);
        if !description.is_empty() {
            info["description"] = description.into();
        }
        let mut document = json!({
            "openapi":"3.1.2", "jsonSchemaDialect":"https://json-schema.org/draft/2020-12/schema",
            "info":info, "paths":paths, "tags":tags,
            "components":{"schemas":components,"responses":{
                "ServiceErrorResponse":operation::framework_response()
            }}
        });
        if !self.servers.is_empty() {
            document["servers"] = self.servers.iter().map(|url| json!({"url":url})).collect();
        }
        if let Some(name) = &self.bearer_auth {
            document["components"]["securitySchemes"] =
                json!({name:{"type":"http","scheme":"bearer"}});
            document["security"] = json!([{name:[]}]);
        }
        Ok(Document(document))
    }

    fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() || self.version.trim().is_empty() {
            return Err(Error::Configuration(
                "title and version must be nonempty".into(),
            ));
        }
        if let Some(name) = &self.bearer_auth {
            schema::component_name(name)?;
        }
        for server in &self.servers {
            let invalid = || {
                Error::Configuration(format!("server URL must be static HTTP(S) or root-relative, without credentials/query/fragment: {server}"))
            };
            if server.contains(['{', '}'])
                || server
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
                || server.starts_with("//")
            {
                return Err(invalid());
            }
            let url = if server.starts_with('/') {
                url::Url::parse("https://spline.invalid")
                    .unwrap()
                    .join(server)
                    .map_err(|_| invalid())?
            } else {
                url::Url::parse(server).map_err(|_| invalid())?
            };
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}
