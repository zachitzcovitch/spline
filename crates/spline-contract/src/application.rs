use std::collections::BTreeMap;

use spline_schema::{Graph, Registry};

use crate::{identifier, ContractBuilder, Error, Operation, ResourceDescription, Result};

/// Resource groups and one shared, direction-aware graph. Group names are client
/// accessors; namespaces remain the source of stable operation identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationDescription {
    documentation: String,
    resources: BTreeMap<String, ResourceDescription>,
    schemas: Graph,
}

impl ApplicationDescription {
    pub fn documentation(&self) -> &str {
        &self.documentation
    }
    pub fn resources(&self) -> &BTreeMap<String, ResourceDescription> {
        &self.resources
    }
    pub fn schemas(&self) -> &Graph {
        &self.schemas
    }
    pub fn operations(&self) -> impl Iterator<Item = &Operation> {
        self.resources
            .values()
            .flat_map(|resource| resource.operations().values())
    }
}

/// Registers each resource transactionally into a shared registry. Registering
/// Rust types together (rather than merging sealed graphs by shape) preserves
/// schema ownership checks, including collisions between identical-looking DTOs.
pub struct ApplicationBuilder {
    documentation: String,
    resources: BTreeMap<String, ResourceDescription>,
    schemas: Registry,
}

impl ApplicationBuilder {
    pub fn new(documentation: impl Into<String>) -> Self {
        Self {
            documentation: documentation.into(),
            resources: BTreeMap::new(),
            schemas: Registry::new(),
        }
    }

    pub fn resource(
        &mut self,
        group: &str,
        namespace: &str,
        documentation: &str,
        register: impl FnOnce(&mut ContractBuilder) -> Result<()>,
    ) -> Result<()> {
        if !identifier(group) {
            return Err(Error::InvalidIdentifier(group.into()));
        }
        if self.resources.contains_key(group) {
            return Err(Error::DuplicateResource(group.into()));
        }
        if self
            .resources
            .values()
            .any(|resource| resource.namespace() == namespace)
        {
            return Err(Error::DuplicateNamespace(namespace.into()));
        }
        let mut candidate = ContractBuilder::new(namespace, documentation)?;
        candidate.schemas = self.schemas.clone();
        register(&mut candidate)?;
        let (resource, schemas) = candidate.into_parts()?;
        self.resources.insert(group.into(), resource);
        self.schemas = schemas;
        Ok(())
    }

    pub fn finish(self) -> Result<ApplicationDescription> {
        if self.resources.is_empty() {
            return Err(Error::EmptyApplication);
        }
        Ok(ApplicationDescription {
            documentation: self.documentation,
            resources: self.resources,
            schemas: self.schemas.finish()?,
        })
    }
}
