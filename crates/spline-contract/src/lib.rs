//! Transport-neutral operation metadata. Construction registers the server's
//! input deserialization shapes and its output/error serialization shapes.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use spline_schema::{Direction, Graph, Presence, Registry, Schema, Shape};

mod application;

pub use application::{ApplicationBuilder, ApplicationDescription};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
    Query,
    Mutation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Operation {
    pub id: String,
    pub name: String,
    pub rust_name: String,
    pub kind: OperationKind,
    pub documentation: String,
    pub input: Shape,
    /// Authored argument order, independent of operation sorting and JSON key order.
    pub arguments: Vec<Argument>,
    pub output: Shape,
    pub error: Shape,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argument {
    pub name: String,
    pub shape: Shape,
    pub optional: bool,
}

/// Sealed through a builder, so identifiers, operations, and schemas agree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractDescription {
    resource: ResourceDescription,
    schemas: Graph,
}

/// A resource's logical operations. An application owns their shared schema graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceDescription {
    namespace: String,
    documentation: String,
    operations: BTreeMap<String, Operation>,
}

impl ResourceDescription {
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    pub fn documentation(&self) -> &str {
        &self.documentation
    }
    pub fn operations(&self) -> &BTreeMap<String, Operation> {
        &self.operations
    }
}

impl ContractDescription {
    pub fn resource(&self) -> &ResourceDescription {
        &self.resource
    }
    pub fn namespace(&self) -> &str {
        self.resource.namespace()
    }
    pub fn documentation(&self) -> &str {
        self.resource.documentation()
    }
    pub fn operations(&self) -> &BTreeMap<String, Operation> {
        self.resource.operations()
    }
    pub fn schemas(&self) -> &Graph {
        &self.schemas
    }
}

pub struct ContractBuilder {
    namespace: String,
    documentation: String,
    operations: BTreeMap<String, Operation>,
    schemas: Registry,
}

impl ContractBuilder {
    pub fn new(namespace: impl Into<String>, documentation: impl Into<String>) -> Result<Self> {
        let namespace = namespace.into();
        if !namespace.split('.').all(identifier) {
            return Err(Error::InvalidIdentifier(namespace));
        }
        Ok(Self {
            namespace,
            documentation: documentation.into(),
            operations: BTreeMap::new(),
            schemas: Registry::new(),
        })
    }

    pub fn operation<Input: Schema, Output: Schema, ApplicationError: Schema>(
        &mut self,
        name: &str,
        rust_name: &str,
        kind: OperationKind,
        documentation: &str,
    ) -> Result<()> {
        if !identifier(name) || !identifier(rust_name) {
            return Err(Error::InvalidIdentifier(format!("{name} / {rust_name}")));
        }
        if self.operations.contains_key(name) {
            return Err(Error::DuplicateOperation(format!(
                "{}.{name}",
                self.namespace
            )));
        }
        // Commit the entire operation together; output failure must not leave
        // successfully registered input definitions behind.
        let mut candidate = self.schemas.clone();
        let input = candidate.register::<Input>(Direction::Deserialize)?;
        let output = candidate.register::<Output>(Direction::Serialize)?;
        let error = candidate.register::<ApplicationError>(Direction::Serialize)?;
        let graph = candidate.clone().finish()?;
        let arguments = match resolve_input(&input, &graph)? {
            Shape::Object(object) => object
                .fields
                .iter()
                .map(|field| Argument {
                    name: field.name.clone(),
                    shape: field.shape.clone(),
                    optional: field.presence == Presence::Optional,
                })
                .collect(),
            Shape::Null => Vec::new(),
            _ => vec![Argument {
                name: "input".into(),
                shape: input.clone(),
                optional: false,
            }],
        };
        self.operations.insert(
            name.into(),
            Operation {
                id: format!("{}.{name}", self.namespace),
                name: name.into(),
                rust_name: rust_name.into(),
                kind,
                documentation: documentation.into(),
                input,
                arguments,
                output,
                error,
            },
        );
        self.schemas = candidate;
        Ok(())
    }

    pub fn finish(self) -> Result<ContractDescription> {
        let (resource, schemas) = self.into_parts()?;
        Ok(ContractDescription {
            resource,
            schemas: schemas.finish()?,
        })
    }

    fn into_parts(self) -> Result<(ResourceDescription, Registry)> {
        if self.operations.is_empty() {
            return Err(Error::EmptyContract);
        }
        Ok((
            ResourceDescription {
                namespace: self.namespace,
                documentation: self.documentation,
                operations: self.operations,
            },
            self.schemas,
        ))
    }
}

fn identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn resolve_input<'a>(mut shape: &'a Shape, graph: &'a Graph) -> Result<&'a Shape> {
    let mut visited = BTreeSet::new();
    while let Shape::Ref(id) = shape {
        if !visited.insert(id) {
            return Err(spline_schema::Error::InvalidShape(
                "cyclic alias in operation input".into(),
            )
            .into());
        }
        shape = &graph
            .definitions()
            .get(id)
            .ok_or_else(|| spline_schema::Error::UnresolvedReference(id.clone()))?
            .shape;
    }
    Ok(shape)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidIdentifier(String),
    DuplicateOperation(String),
    DuplicateResource(String),
    DuplicateNamespace(String),
    EmptyContract,
    EmptyApplication,
    Schema(spline_schema::Error),
}

impl From<spline_schema::Error> for Error {
    fn from(error: spline_schema::Error) -> Self {
        Self::Schema(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(id) => write!(f, "invalid contract identifier: {id}"),
            Self::DuplicateOperation(id) => write!(f, "duplicate operation: {id}"),
            Self::DuplicateResource(name) => write!(f, "duplicate application resource: {name}"),
            Self::DuplicateNamespace(name) => write!(f, "duplicate resource namespace: {name}"),
            Self::EmptyContract => f.write_str("a contract must contain at least one operation"),
            Self::EmptyApplication => {
                f.write_str("an application must contain at least one resource")
            }
            Self::Schema(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Schema(error) => Some(error),
            _ => None,
        }
    }
}
