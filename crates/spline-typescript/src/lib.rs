//! Deterministic wire types, contract/application signatures, and HTTP clients.

#![forbid(unsafe_code)]

mod contract;
mod http;
mod names;
mod render;
mod validate;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use spline_schema::{DefinitionId, Direction, Graph, Shape};

/// A named entry point, including inline shapes that have no graph definition.
#[derive(Clone, Debug)]
pub struct Root {
    pub symbol: String,
    pub direction: Direction,
    pub shape: Shape,
    pub documentation: String,
}

impl Root {
    pub fn new(symbol: impl Into<String>, direction: Direction, shape: Shape) -> Self {
        Self {
            symbol: symbol.into(),
            direction,
            shape,
            documentation: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    pub source: String,
    /// Emitted public names. Equivalent directions can map to one declaration.
    /// Contract exports include reachable DTOs, excluding dispatch wrappers.
    pub symbols: BTreeMap<DefinitionId, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidSymbol(String),
    SymbolCollision(String),
    UnknownOverride(DefinitionId),
    MissingReference(DefinitionId),
    WrongDirection(DefinitionId),
    InvalidShape(String),
    UnguardedRecursion(Vec<DefinitionId>),
    MissingHttpBinding(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSymbol(name) => write!(f, "invalid or reserved TypeScript symbol: {name}"),
            Self::SymbolCollision(name) => write!(
                f,
                "TypeScript symbol collision: {name}; assign an explicit name"
            ),
            Self::UnknownOverride(id) => {
                write!(f, "name override refers to an absent definition: {id}")
            }
            Self::MissingReference(id) => write!(f, "missing definition: {id}"),
            Self::WrongDirection(id) => write!(f, "root or definition crosses directions: {id}"),
            Self::InvalidShape(message) => f.write_str(message),
            Self::UnguardedRecursion(cycle) => write!(
                f,
                "TypeScript cannot represent an unguarded alias cycle: {cycle:?}"
            ),
            Self::MissingHttpBinding(id) => write!(f, "operation has no HTTP binding: {id}"),
        }
    }
}

impl std::error::Error for Error {}

/// Explicit overrides are scoped to a logical definition and its direction.
#[derive(Clone, Debug, Default)]
pub struct Typescript {
    overrides: BTreeMap<DefinitionId, String>,
}

impl Typescript {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, id: DefinitionId, symbol: impl Into<String>) -> Self {
        self.overrides.insert(id, symbol.into());
        self
    }

    /// Emit short DTO names plus optional root aliases, sorted by final symbol.
    /// Equivalent directions share a declaration; differing shapes use suffixes.
    /// There are no filesystem effects; callers decide where/how to save source.
    pub fn export(&self, graph: &Graph, roots: &[Root]) -> Result<Artifact, Error> {
        self.export_selected(graph, roots, &graph.definitions().keys().cloned().collect())
    }

    /// Contracts select DTOs reachable from their public signatures, excluding
    /// generated argument wrappers. Runtime schemas keep their logical identities.
    pub(crate) fn export_selected(
        &self,
        graph: &Graph,
        roots: &[Root],
        selected: &BTreeSet<DefinitionId>,
    ) -> Result<Artifact, Error> {
        for id in self.overrides.keys() {
            if !graph.definitions().contains_key(id) {
                return Err(Error::UnknownOverride(id.clone()));
            }
        }
        for (id, definition) in graph.definitions() {
            validate::shape(&definition.shape, id.direction, graph)?;
        }
        let symbols = names::assign(graph, selected, &self.overrides)?;
        let mut declarations = BTreeMap::new();
        for (id, definition) in graph.definitions() {
            let Some(symbol) = symbols.get(id) else {
                continue;
            };
            validate::symbol(symbol)?;
            // A shared directional symbol is proved equivalent by names::assign.
            // Prefer the serialization description for its declaration/docs.
            declarations.insert(
                symbol.clone(),
                (&definition.shape, definition.documentation.as_str()),
            );
        }
        for root in roots {
            validate::symbol(&root.symbol)?;
            if declarations
                .insert(
                    root.symbol.clone(),
                    (&root.shape, root.documentation.as_str()),
                )
                .is_some()
            {
                return Err(Error::SymbolCollision(root.symbol.clone()));
            }
            validate::shape(&root.shape, root.direction, graph)?;
        }
        validate::cycles(graph)?;

        let mut source = String::from(
            "// Generated by Spline. Do not edit.\n\
             // Public DTO names; Input/Output suffixes distinguish differing wire shapes.\n\n",
        );
        // Keep an empty artifact a module as well.
        if declarations.is_empty() {
            source.push_str("export {};\n");
        }
        for (symbol, (shape, docs)) in declarations {
            source.push_str(&render::docs(docs, 0));
            source.push_str("export type ");
            source.push_str(&symbol);
            source.push_str(" = ");
            source.push_str(&render::shape(shape, &symbols, 0)?);
            source.push_str(";\n\n");
        }
        if source.ends_with("\n\n") {
            source.pop();
        }
        Ok(Artifact { source, symbols })
    }
}
