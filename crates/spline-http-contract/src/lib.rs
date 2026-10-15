//! Sealed HTTP bindings layered over neutral contract/application descriptions.
//! This crate has no HTTP runtime dependencies.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;

use spline_contract as core;
use spline_schema::Graph;

mod errors;
mod plan;
pub use errors::{
    application_error_schema, framework_error_schema, ErrorMapping, ErrorResponse, ErrorResponses,
    ERROR_HEADER, MAX_BODY_BYTES,
};
pub use plan::{HttpBinding, HttpMethod, HttpPlan, Scalar, UrlParameter};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Contract(core::Error),
    Binding(String),
}
impl From<core::Error> for Error {
    fn from(error: core::Error) -> Self {
        Self::Contract(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => error.fmt(f),
            Self::Binding(message) => write!(f, "invalid HTTP binding: {message}"),
        }
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractDescription {
    contract: core::ContractDescription,
    plans: BTreeMap<String, HttpPlan>,
}
impl ContractDescription {
    pub fn contract(&self) -> &core::ContractDescription {
        &self.contract
    }
    pub fn plans(&self) -> &BTreeMap<String, HttpPlan> {
        &self.plans
    }

    /// Attach a selected policy's application responses transactionally.
    pub fn errors(&mut self, id: &str, mapping: ErrorMapping) -> Result<()> {
        let operation = self
            .contract
            .operations()
            .values()
            .find(|op| op.id == id)
            .ok_or_else(|| Error::Binding(format!("unknown operation {id}")))?;
        let responses = errors::responses(mapping, &operation.error, self.contract.schemas())
            .map_err(|error| match error {
                Error::Binding(message) => Error::Binding(format!("{id}: {message}")),
                error => error,
            })?;
        self.plans.get_mut(id).expect("sealed binding").errors = responses;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationDescription {
    application: core::ApplicationDescription,
    plans: BTreeMap<String, HttpPlan>,
}
impl ApplicationDescription {
    pub fn application(&self) -> &core::ApplicationDescription {
        &self.application
    }
    pub fn plans(&self) -> &BTreeMap<String, HttpPlan> {
        &self.plans
    }

    /// Attach a selected policy's application responses transactionally.
    pub fn errors(&mut self, id: &str, mapping: ErrorMapping) -> Result<()> {
        let operation = self
            .application
            .operations()
            .find(|op| op.id == id)
            .ok_or_else(|| Error::Binding(format!("unknown operation {id}")))?;
        let responses = errors::responses(mapping, &operation.error, self.application.schemas())
            .map_err(|error| match error {
                Error::Binding(message) => Error::Binding(format!("{id}: {message}")),
                error => error,
            })?;
        self.plans.get_mut(id).expect("sealed binding").errors = responses;
        Ok(())
    }
}

pub struct ContractBuilder {
    contract: core::ContractDescription,
    plans: BTreeMap<String, HttpPlan>,
}
impl ContractBuilder {
    pub fn new(contract: core::ContractDescription) -> Self {
        Self {
            contract,
            plans: BTreeMap::new(),
        }
    }
    /// Bind a fully-qualified logical operation ID.
    pub fn bind(&mut self, id: &str, binding: HttpBinding) -> Result<()> {
        let operation = self
            .contract
            .operations()
            .values()
            .find(|op| op.id == id)
            .ok_or_else(|| Error::Binding(format!("unknown operation {id}")))?;
        bind(&mut self.plans, operation, binding, self.contract.schemas())
    }
    pub fn finish(self) -> Result<ContractDescription> {
        complete(self.contract.operations().values(), &self.plans)?;
        Ok(ContractDescription {
            contract: self.contract,
            plans: self.plans,
        })
    }
}

pub struct ApplicationBuilder {
    application: core::ApplicationDescription,
    plans: BTreeMap<String, HttpPlan>,
}
impl ApplicationBuilder {
    pub fn new(application: core::ApplicationDescription) -> Self {
        Self {
            application,
            plans: BTreeMap::new(),
        }
    }
    pub fn bind(&mut self, id: &str, binding: HttpBinding) -> Result<()> {
        let operation = self
            .application
            .operations()
            .find(|op| op.id == id)
            .ok_or_else(|| Error::Binding(format!("unknown operation {id}")))?;
        bind(
            &mut self.plans,
            operation,
            binding,
            self.application.schemas(),
        )
    }
    pub fn finish(self) -> Result<ApplicationDescription> {
        complete(self.application.operations(), &self.plans)?;
        Ok(ApplicationDescription {
            application: self.application,
            plans: self.plans,
        })
    }
}

fn bind(
    plans: &mut BTreeMap<String, HttpPlan>,
    operation: &core::Operation,
    binding: HttpBinding,
    graph: &Graph,
) -> Result<()> {
    if plans.contains_key(&operation.id) {
        return Err(Error::Binding(format!(
            "duplicate binding for {}",
            operation.id
        )));
    }
    let candidate = plan::plan(operation, binding, graph)?;
    for (id, other) in plans.iter() {
        if plan::conflicts(other, &candidate) {
            return Err(Error::Binding(format!(
                "conflicting HTTP routes for {id} and {}: {}",
                operation.id, candidate.binding.path
            )));
        }
    }
    plans.insert(operation.id.clone(), candidate);
    Ok(())
}

fn complete<'a>(
    operations: impl Iterator<Item = &'a core::Operation>,
    plans: &BTreeMap<String, HttpPlan>,
) -> Result<()> {
    for operation in operations {
        if !plans.contains_key(&operation.id) {
            return Err(Error::Binding(format!(
                "{} has no HTTP binding",
                operation.id
            )));
        }
    }
    Ok(())
}

fn identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
