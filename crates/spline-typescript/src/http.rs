use std::collections::BTreeSet;
use std::fmt::Write;

use spline_contract::ResourceDescription;
use spline_http_contract::{ApplicationDescription, ContractDescription, HttpMethod, HttpPlan};
use spline_schema::{Graph, Presence, Shape, UnknownFields};
use std::collections::BTreeMap;

use crate::{contract, render, Artifact, Error, Typescript};

impl Typescript {
    pub fn export_http_client(
        &self,
        contract: &ContractDescription,
        name: &str,
    ) -> Result<Artifact, Error> {
        self.export_http_resources(
            contract.contract().schemas(),
            &[(None, contract.contract().resource())],
            contract.contract().documentation(),
            name,
            contract.plans(),
        )
    }

    /// Generate one grouped client sharing a base URL, fetch implementation,
    /// default headers, response policy, and runtime schema table.
    pub fn export_http_application(
        &self,
        application: &ApplicationDescription,
        name: &str,
    ) -> Result<Artifact, Error> {
        self.export_http_resources(
            application.application().schemas(),
            &contract::application_resources(application.application()),
            application.application().documentation(),
            name,
            application.plans(),
        )
    }

    fn export_http_resources(
        &self,
        schemas: &Graph,
        resources: &[(Option<&str>, &ResourceDescription)],
        documentation: &str,
        name: &str,
        plans: &BTreeMap<String, HttpPlan>,
    ) -> Result<Artifact, Error> {
        let void_outputs = plans
            .iter()
            .filter(|(_, plan)| plan.binding.success_status == 204)
            .map(|(id, _)| id.clone())
            .collect();
        let mut artifact =
            self.export_resources(schemas, resources, documentation, name, &void_outputs)?;
        let occupied: BTreeSet<_> = artifact.symbols.values().cloned().collect();
        for symbol in [
            format!("{name}HttpFailure"),
            format!("{name}HttpOptions"),
            format!("create{name}Client"),
            format!("{name}Client"),
            "__SplineWire".into(),
            "__SplinePlan".into(),
        ] {
            if occupied.contains(&symbol) {
                return Err(Error::SymbolCollision(symbol));
            }
        }
        let mut definitions = String::from("{\n");
        for (id, definition) in schemas.definitions() {
            writeln!(
                definitions,
                "    [{}]: {},",
                render::string(&id.to_string()),
                wire(&definition.shape)
            )
            .unwrap();
        }
        definitions.push_str("  }");
        let mut methods = String::new();
        for (group, resource) in resources {
            let indent = if group.is_some() { "      " } else { "    " };
            if let Some(group) = group {
                writeln!(methods, "    [{}]: {{", render::string(group)).unwrap();
            }
            for operation in resource.operations().values() {
                let plan = plans
                    .get(&operation.id)
                    .ok_or_else(|| Error::MissingHttpBinding(operation.id.clone()))?;
                let method = match plan.binding.method {
                    HttpMethod::Get => "GET",
                    HttpMethod::Post => "POST",
                    HttpMethod::Put => "PUT",
                    HttpMethod::Patch => "PATCH",
                    HttpMethod::Delete => "DELETE",
                };
                let body = plan
                    .binding
                    .body
                    .as_ref()
                    .map(|name| render::string(name))
                    .unwrap_or_else(|| "null".into());
                let paths = plan
                    .path_parameters
                    .iter()
                    .map(|p| render::string(&p.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                let queries = plan
                    .query_parameters
                    .iter()
                    .map(|p| format!("[{}, {}]", render::string(&p.name), p.optional))
                    .collect::<Vec<_>>()
                    .join(", ");
                let parameters = contract::parameters(operation, &artifact.symbols)?;
                let fields = operation
                    .arguments
                    .iter()
                    .zip(contract::parameter_names(operation))
                    .map(|(arg, name)| format!("[{}]: {}", render::string(&arg.name), name))
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(methods, "{indent}[{}]: ({parameters}) => call<{}, {}>({{\n{indent}  method: {method:?}, path: {}, body: {body}, status: {}, paths: [{paths}], queries: [{queries}],\n{indent}  input: {}, output: {}, error: {},\n{indent}}}, {{ {fields} }}),",
                render::string(&operation.name), contract::output(operation, &artifact.symbols, &void_outputs)?, render::shape(&operation.error, &artifact.symbols, 1)?,
                render::string(&plan.binding.path), plan.binding.success_status,
                wire(&operation.input), wire(&operation.output), wire(&operation.error),
            ).unwrap();
            }
            if group.is_some() {
                methods.push_str("    },\n");
            }
        }
        let template = include_str!("http-runtime.ts").replace("__PREFIX__", name);
        let (prefix, rest) = template.split_once("/*__DEFINITIONS__*/").unwrap();
        let (middle, suffix) = rest.split_once("/*__METHODS__*/").unwrap();
        let runtime = format!("{prefix}{definitions}{middle}{methods}{suffix}");
        artifact.source.push('\n');
        artifact.source.push_str(&runtime);
        Ok(artifact)
    }
}

fn wire(shape: &Shape) -> String {
    match shape {
        Shape::Null => "[\"null\"]".into(),
        Shape::String => "[\"string\"]".into(),
        Shape::Boolean => "[\"boolean\"]".into(),
        Shape::Integer { signed, bits } => format!("[\"integer\", {signed}, {bits}]"),
        Shape::Literal(value) => format!("[\"literal\", {}]", render::string(value)),
        Shape::Ref(id) => format!("[\"ref\", {}]", render::string(&id.to_string())),
        Shape::Array(value) => format!("[\"array\", {}]", wire(value)),
        Shape::Map(value) => format!("[\"map\", {}]", wire(value)),
        Shape::Tuple(values) | Shape::Union(values) => {
            let kind = if matches!(shape, Shape::Tuple(_)) {
                "tuple"
            } else {
                "union"
            };
            let values = values.iter().map(wire).collect::<Vec<_>>().join(", ");
            format!("[{kind:?}, [{values}]]")
        }
        Shape::Object(object) => {
            let fields = object
                .fields
                .iter()
                .map(|f| {
                    format!(
                        "[{}, {}, {}]",
                        render::string(&f.name),
                        wire(&f.shape),
                        f.presence == Presence::Optional
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "[\"object\", {}, [{fields}]]",
                object.unknown_fields == UnknownFields::Ignore
            )
        }
    }
}
