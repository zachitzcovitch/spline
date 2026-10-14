use std::collections::BTreeSet;
use std::fmt::Write;

use spline_contract::{
    ApplicationDescription, ContractDescription, OperationKind, ResourceDescription,
};
use spline_schema::{DefinitionId, Graph, Shape};

use crate::{render, validate, Artifact, Error, Typescript};

impl Typescript {
    /// Emit a contract's wire types, operation map, and callable type surface.
    /// `Failure` is an explicit extension point for a future binding's errors;
    /// this emits signatures only and does not supply a transport implementation.
    pub fn export_contract(
        &self,
        contract: &ContractDescription,
        name: &str,
    ) -> Result<Artifact, Error> {
        self.export_resources(
            contract.schemas(),
            &[(None, contract.resource())],
            contract.documentation(),
            name,
            &BTreeSet::new(),
        )
    }

    /// Emit shared DTOs once, a flat operation map keyed by stable identity, and
    /// positional call signatures grouped by the application's resource names.
    pub fn export_application(
        &self,
        application: &ApplicationDescription,
        name: &str,
    ) -> Result<Artifact, Error> {
        self.export_resources(
            application.schemas(),
            &application_resources(application),
            application.documentation(),
            name,
            &BTreeSet::new(),
        )
    }

    pub(crate) fn export_resources(
        &self,
        schemas: &Graph,
        resources: &[(Option<&str>, &ResourceDescription)],
        documentation: &str,
        name: &str,
        void_outputs: &BTreeSet<String>,
    ) -> Result<Artifact, Error> {
        validate::symbol(name)?;
        let calls = format!("{name}Calls");
        let operations = format!("{name}Operations");
        let result = format!("{name}Result");
        let mut selected = BTreeSet::new();
        for operation in resources.iter().flat_map(|(_, r)| r.operations().values()) {
            select(input_shape(operation, schemas)?, schemas, &mut selected)?;
            select(&operation.output, schemas, &mut selected)?;
            select(&operation.error, schemas, &mut selected)?;
        }
        let mut artifact = self.export_selected(schemas, &[], &selected)?;
        let occupied: BTreeSet<_> = artifact.symbols.values().map(String::as_str).collect();
        // Promise is a global type dependency and Failure a local type parameter.
        // Reject shadowing instead of generating changed or invalid signatures.
        for symbol in [&calls, &operations, &result, "Promise", "Failure"] {
            if occupied.contains(symbol) {
                return Err(Error::SymbolCollision(symbol.into()));
            }
        }
        artifact.source.push('\n');
        artifact.source.push_str(&render::docs(documentation, 0));
        writeln!(artifact.source, "export type {operations} = {{").unwrap();
        for operation in resources.iter().flat_map(|(_, r)| r.operations().values()) {
            artifact
                .source
                .push_str(&render::docs(&operation.documentation, 1));
            let kind = match operation.kind {
                OperationKind::Query => "query",
                OperationKind::Mutation => "mutation",
            };
            writeln!(
                artifact.source,
                "  {}: {{ kind: {}; input: {}; output: {}; error: {} }};",
                render::string(&operation.id),
                render::string(kind),
                render::shape(input_shape(operation, schemas)?, &artifact.symbols, 1)?,
                output(operation, &artifact.symbols, void_outputs)?,
                render::shape(&operation.error, &artifact.symbols, 1)?
            )
            .unwrap();
        }
        artifact.source.push_str("};\n\n");
        writeln!(artifact.source, "export type {result}<Value, ApplicationError, Failure = never> =\n  | {{ ok: true; value: Value }}\n  | {{ ok: false; error: {{ kind: \"application\"; value: ApplicationError }} | Failure }};\n").unwrap();
        artifact.source.push_str("/** Typed call signatures; a binding supplies execution and any extra Failure union. */\n");
        writeln!(artifact.source, "export type {calls}<Failure = never> = {{").unwrap();
        for (group, resource) in resources {
            let depth = if let Some(group) = group {
                artifact
                    .source
                    .push_str(&render::docs(resource.documentation(), 1));
                writeln!(artifact.source, "  {}: {{", render::string(group)).unwrap();
                2
            } else {
                1
            };
            let indent = "  ".repeat(depth);
            for operation in resource.operations().values() {
                artifact
                    .source
                    .push_str(&render::docs(&operation.documentation, depth));
                let parameters = parameters(operation, &artifact.symbols)?;
                writeln!(
                    artifact.source,
                    "{indent}{}({parameters}): Promise<{result}<{}, {}, Failure>>;",
                    render::property(&operation.name),
                    output(operation, &artifact.symbols, void_outputs)?,
                    render::shape(&operation.error, &artifact.symbols, depth)?
                )
                .unwrap();
            }
            if group.is_some() {
                artifact.source.push_str("  };\n");
            }
        }
        artifact.source.push_str("};\n");
        Ok(artifact)
    }
}

pub(crate) fn application_resources(
    application: &ApplicationDescription,
) -> Vec<(Option<&str>, &ResourceDescription)> {
    application
        .resources()
        .iter()
        .map(|(group, resource)| (Some(group.as_str()), resource))
        .collect()
}

pub(crate) fn parameter_names(operation: &spline_contract::Operation) -> Vec<String> {
    let authored: BTreeSet<_> = operation
        .arguments
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    operation
        .arguments
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            if validate::symbol(&argument.name).is_ok() && argument.name != "call" {
                argument.name.clone()
            } else {
                let mut fallback = format!("arg{index}");
                while authored.contains(fallback.as_str()) {
                    fallback.push('_');
                }
                fallback
            }
        })
        .collect()
}

pub(crate) fn parameters(
    operation: &spline_contract::Operation,
    symbols: &std::collections::BTreeMap<spline_schema::DefinitionId, String>,
) -> Result<String, Error> {
    operation
        .arguments
        .iter()
        .zip(parameter_names(operation))
        .map(|(argument, name)| {
            Ok(format!(
                "{}: {}",
                name,
                render::shape(&argument.shape, symbols, 1)?
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parameters| parameters.join(", "))
}

pub(crate) fn output(
    operation: &spline_contract::Operation,
    symbols: &std::collections::BTreeMap<DefinitionId, String>,
    void_outputs: &BTreeSet<String>,
) -> Result<String, Error> {
    if void_outputs.contains(&operation.id) {
        Ok("void".into())
    } else {
        render::shape(&operation.output, symbols, 1)
    }
}

fn input_shape<'a>(
    operation: &'a spline_contract::Operation,
    graph: &'a Graph,
) -> Result<&'a Shape, Error> {
    if let Shape::Ref(id) = &operation.input {
        if id.name.path() == format!("{}.Args", operation.id) {
            return graph
                .definitions()
                .get(id)
                .map(|d| &d.shape)
                .ok_or_else(|| Error::MissingReference(id.clone()));
        }
    }
    Ok(&operation.input)
}

fn select(
    shape: &Shape,
    graph: &Graph,
    selected: &mut BTreeSet<DefinitionId>,
) -> Result<(), Error> {
    match shape {
        Shape::Ref(id) => {
            if selected.insert(id.clone()) {
                let definition = graph
                    .definitions()
                    .get(id)
                    .ok_or_else(|| Error::MissingReference(id.clone()))?;
                select(&definition.shape, graph, selected)?;
            }
        }
        Shape::Array(child) | Shape::Map(child) => select(child, graph, selected)?,
        Shape::Tuple(children) | Shape::Union(children) => {
            for child in children {
                select(child, graph, selected)?;
            }
        }
        Shape::Object(object) => {
            for field in &object.fields {
                select(&field.shape, graph, selected)?;
            }
        }
        _ => {}
    }
    Ok(())
}
