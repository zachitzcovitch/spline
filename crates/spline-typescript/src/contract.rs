use std::collections::BTreeSet;
use std::fmt::Write;

use spline_contract::{
    ApplicationDescription, ContractDescription, OperationKind, ResourceDescription,
};
use spline_schema::{Direction, Graph};

use crate::{render, validate, Artifact, Error, Root, Typescript};

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
        )
    }

    pub(crate) fn export_resources(
        &self,
        schemas: &Graph,
        resources: &[(Option<&str>, &ResourceDescription)],
        documentation: &str,
        name: &str,
    ) -> Result<Artifact, Error> {
        validate::symbol(name)?;
        let calls = format!("{name}Calls");
        let operations = format!("{name}Operations");
        let mut roots = Vec::new();
        let mut void_roots = BTreeSet::new();
        for operation in resources.iter().flat_map(|(_, r)| r.operations().values()) {
            let stem = operation.id.replace('.', "_");
            if operation
                .http
                .as_ref()
                .is_some_and(|plan| plan.binding.success_status == 204)
            {
                void_roots.insert(format!("{stem}_Output"));
            }
            roots.push(Root::new(
                format!("{stem}_Input"),
                Direction::Deserialize,
                operation.input.clone(),
            ));
            roots.push(Root::new(
                format!("{stem}_Output"),
                Direction::Serialize,
                operation.output.clone(),
            ));
            roots.push(Root::new(
                format!("{stem}_Error"),
                Direction::Serialize,
                operation.error.clone(),
            ));
        }
        let mut artifact = self.export_roots(schemas, &roots, &void_roots)?;
        let occupied: BTreeSet<_> = artifact
            .symbols
            .values()
            .map(String::as_str)
            .chain(roots.iter().map(|root| root.symbol.as_str()))
            .collect();
        // Promise is a global type dependency and Failure a local type parameter.
        // Reject shadowing instead of generating changed or invalid signatures.
        for symbol in [&calls, &operations, "Promise", "Failure"] {
            if occupied.contains(symbol) {
                return Err(Error::SymbolCollision(symbol.into()));
            }
        }
        artifact.source.push('\n');
        artifact.source.push_str(&render::docs(documentation, 0));
        writeln!(artifact.source, "export type {operations} = {{").unwrap();
        for operation in resources.iter().flat_map(|(_, r)| r.operations().values()) {
            let stem = operation.id.replace('.', "_");
            artifact
                .source
                .push_str(&render::docs(&operation.documentation, 1));
            let kind = match operation.kind {
                OperationKind::Query => "query",
                OperationKind::Mutation => "mutation",
            };
            writeln!(artifact.source, "  {}: {{ kind: {}; input: {stem}_Input; output: {stem}_Output; error: {stem}_Error }};",
                render::string(&operation.id), render::string(kind)).unwrap();
        }
        artifact.source.push_str("};\n\n");
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
                let stem = operation.id.replace('.', "_");
                artifact
                    .source
                    .push_str(&render::docs(&operation.documentation, depth));
                let parameters = parameters(operation, &artifact.symbols)?;
                writeln!(artifact.source, "{indent}{}: ({parameters}) => Promise<\n{indent}  | {{ ok: true; value: {stem}_Output }}\n{indent}  | {{ ok: false; error: {{ kind: \"application\"; value: {stem}_Error }} | Failure }}\n{indent}>;",
                    render::string(&operation.name)).unwrap();
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

pub(crate) fn parameter_name(name: &str, index: usize) -> String {
    // Prefixing avoids keywords and collisions with other authored argument names.
    format!("arg{index}_{name}")
}

pub(crate) fn parameters(
    operation: &spline_contract::Operation,
    symbols: &std::collections::BTreeMap<spline_schema::DefinitionId, String>,
) -> Result<String, Error> {
    operation
        .arguments
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            Ok(format!(
                "{}: {}",
                parameter_name(&argument.name, index),
                render::shape(&argument.shape, symbols, 1)?
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parameters| parameters.join(", "))
}
