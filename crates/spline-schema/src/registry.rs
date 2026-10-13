use std::any::{type_name, TypeId};
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::{Definition, DefinitionId, Direction, Error, Name, Result, Schema, Shape};

type Key = (TypeId, Direction);

#[derive(Clone)]
enum State {
    Building(Option<DefinitionId>),
    Ready(Shape),
}

/// Registration is transactional per root. The prototype uses cloning for
/// clarity; a production implementation could journal the inserted entries.
#[derive(Clone, Default)]
pub struct Registry {
    states: HashMap<Key, State>,
    owners: BTreeMap<DefinitionId, (TypeId, &'static str)>,
    definitions: BTreeMap<DefinitionId, Definition>,
    failure: Option<Error>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Schema>(&mut self, direction: Direction) -> Result<Shape> {
        let mut candidate = self.clone();
        let root = candidate.build::<T>(direction)?;
        if let Some(error) = candidate.failure.take() {
            return Err(error);
        }
        candidate.validate()?;
        validate_shape(&root, direction, &candidate.definitions)?;
        *self = candidate;
        Ok(root)
    }

    /// Seal the completed definitions. Process-local TypeIds are discarded.
    pub fn finish(self) -> Result<Graph> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        self.validate()?;
        Ok(Graph {
            definitions: self.definitions,
        })
    }

    fn build<T: Schema>(&mut self, direction: Direction) -> Result<Shape> {
        let result = self.build_inner::<T>(direction);
        if let Err(error) = &result {
            self.record_failure(error.clone());
        }
        result
    }

    fn build_inner<T: Schema>(&mut self, direction: Direction) -> Result<Shape> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let key = (TypeId::of::<T>(), direction);
        if let Some(state) = self.states.get(&key) {
            return match state {
                State::Ready(shape) => Ok(shape.clone()),
                State::Building(Some(id)) => Ok(Shape::Ref(id.clone())),
                State::Building(None) => Err(Error::InlineRecursion(T::info().name)),
            };
        }

        let info = T::info();
        info.name.validate()?;
        let definition = if info.inline {
            None
        } else {
            let id = DefinitionId {
                name: info.name,
                direction,
            };
            if let Some((owner, first)) = self.owners.get(&id) {
                if *owner != TypeId::of::<T>() {
                    return Err(Error::NameCollision {
                        id,
                        first,
                        second: type_name::<T>(),
                    });
                }
            }
            self.owners
                .insert(id.clone(), (TypeId::of::<T>(), type_name::<T>()));
            Some(id)
        };

        // Reserve before asking the type to describe its children.
        self.states.insert(key, State::Building(definition.clone()));
        let body = T::describe(&mut Context {
            registry: self,
            direction,
        })?;
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let root = match definition {
            Some(id) => {
                self.definitions.insert(
                    id.clone(),
                    Definition {
                        shape: body,
                        documentation: info.documentation,
                    },
                );
                Shape::Ref(id)
            }
            None => body,
        };
        self.states.insert(key, State::Ready(root.clone()));
        Ok(root)
    }

    fn record_failure(&mut self, error: Error) {
        if self.failure.is_none() {
            self.failure = Some(error);
        }
    }

    fn validate(&self) -> Result<()> {
        if self
            .states
            .values()
            .any(|state| matches!(state, State::Building(_)))
        {
            return Err(Error::InvalidShape("unfinished schema registration".into()));
        }
        for (id, definition) in &self.definitions {
            validate_shape(&definition.shape, id.direction, &self.definitions)?;
        }
        // Validate cached inline descriptions as well as named definitions.
        for ((_, direction), state) in &self.states {
            if let State::Ready(shape) = state {
                validate_shape(shape, *direction, &self.definitions)?;
            }
        }
        Ok(())
    }
}

pub struct Context<'a> {
    registry: &'a mut Registry,
    direction: Direction,
}

impl Context<'_> {
    pub fn direction(&self) -> Direction {
        self.direction
    }

    pub fn schema<T: Schema>(&mut self) -> Result<Shape> {
        self.registry.build::<T>(self.direction)
    }

    pub(crate) fn fail<T>(&mut self, error: Error) -> Result<T> {
        self.registry.record_failure(error.clone());
        Err(error)
    }
}

/// Stable, ordered descriptions; no parser types or process-local identities.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Graph {
    definitions: BTreeMap<DefinitionId, Definition>,
}

impl Graph {
    pub fn definitions(&self) -> &BTreeMap<DefinitionId, Definition> {
        &self.definitions
    }

    pub fn get(&self, name: impl Into<Name>, direction: Direction) -> Option<&Definition> {
        self.definitions.get(&DefinitionId {
            name: name.into(),
            direction,
        })
    }
}

fn validate_shape(
    shape: &Shape,
    direction: Direction,
    definitions: &BTreeMap<DefinitionId, Definition>,
) -> Result<()> {
    // Do not follow references: recursive definitions are checked once each.
    match shape {
        Shape::Ref(id) => {
            if id.direction != direction {
                return Err(Error::DirectionMismatch(id.clone()));
            }
            if !definitions.contains_key(id) {
                return Err(Error::UnresolvedReference(id.clone()));
            }
        }
        Shape::Array(value) | Shape::Map(value) => validate_shape(value, direction, definitions)?,
        Shape::Tuple(values) | Shape::Union(values) => {
            if matches!(shape, Shape::Union(_)) && values.is_empty() {
                return Err(Error::InvalidShape("empty union".into()));
            }
            for value in values {
                validate_shape(value, direction, definitions)?;
            }
        }
        Shape::Object(object) => {
            let mut names = BTreeSet::new();
            for field in &object.fields {
                if !names.insert(&field.name) {
                    return Err(Error::InvalidShape(format!(
                        "duplicate field: {}",
                        field.name
                    )));
                }
                validate_shape(&field.shape, direction, definitions)?;
            }
        }
        Shape::Integer { bits, .. } if !matches!(bits, 8 | 16 | 32) => {
            return Err(Error::Unsupported(format!("{bits}-bit JSON integer")));
        }
        _ => {}
    }
    Ok(())
}
