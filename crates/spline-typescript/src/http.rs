use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use spline_contract::{
    http::HttpMethod, ApplicationDescription, ContractDescription, ResourceDescription,
};
use spline_schema::{DefinitionId, Graph, Presence, Shape, UnknownFields};

use crate::{contract, render, Artifact, Error, Typescript};

impl Typescript {
    pub fn export_http_client(
        &self,
        contract: &ContractDescription,
        name: &str,
    ) -> Result<Artifact, Error> {
        self.export_http_resources(
            contract.schemas(),
            &[(None, contract.resource())],
            contract.documentation(),
            name,
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
            application.schemas(),
            &contract::application_resources(application),
            application.documentation(),
            name,
        )
    }

    fn export_http_resources(
        &self,
        schemas: &Graph,
        resources: &[(Option<&str>, &ResourceDescription)],
        documentation: &str,
        name: &str,
    ) -> Result<Artifact, Error> {
        let mut artifact = self.export_resources(schemas, resources, documentation, name)?;
        let occupied: BTreeSet<_> = artifact
            .symbols
            .values()
            .cloned()
            .chain(
                resources
                    .iter()
                    .flat_map(|(_, r)| r.operations().values())
                    .flat_map(|op| {
                        ["Input", "Output", "Error"]
                            .map(|suffix| format!("{}_{suffix}", op.id.replace('.', "_")))
                    }),
            )
            .collect();
        for symbol in [
            format!("{name}HttpFailure"),
            format!("{name}HttpOptions"),
            format!("create{name}Client"),
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
                render::string(&artifact.symbols[id]),
                wire(&definition.shape, &artifact.symbols)?
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
                let plan = operation
                    .http
                    .as_ref()
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
                    .enumerate()
                    .map(|(index, arg)| {
                        format!(
                            "[{}]: {}",
                            render::string(&arg.name),
                            contract::parameter_name(&arg.name, index)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let stem = operation.id.replace('.', "_");
                writeln!(methods, "{indent}[{}]: ({parameters}) => call<{stem}_Output, {stem}_Error>({{\n{indent}  method: {method:?}, path: {}, body: {body}, status: {}, paths: [{paths}], queries: [{queries}],\n{indent}  input: {}, output: {}, error: {},\n{indent}}}, {{ {fields} }}),",
                render::string(&operation.name), render::string(&plan.binding.path), plan.binding.success_status,
                wire(&operation.input, &artifact.symbols)?, wire(&operation.output, &artifact.symbols)?, wire(&operation.error, &artifact.symbols)?,
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

fn wire(shape: &Shape, symbols: &BTreeMap<DefinitionId, String>) -> Result<String, Error> {
    Ok(match shape {
        Shape::Null => "[\"null\"]".into(),
        Shape::String => "[\"string\"]".into(),
        Shape::Boolean => "[\"boolean\"]".into(),
        Shape::Integer { signed, bits } => format!("[\"integer\", {signed}, {bits}]"),
        Shape::Literal(value) => format!("[\"literal\", {}]", render::string(value)),
        Shape::Ref(id) => format!(
            "[\"ref\", {}]",
            render::string(
                symbols
                    .get(id)
                    .ok_or_else(|| Error::MissingReference(id.clone()))?
            )
        ),
        Shape::Array(value) => format!("[\"array\", {}]", wire(value, symbols)?),
        Shape::Map(value) => format!("[\"map\", {}]", wire(value, symbols)?),
        Shape::Tuple(values) | Shape::Union(values) => {
            let kind = if matches!(shape, Shape::Tuple(_)) {
                "tuple"
            } else {
                "union"
            };
            let values = values
                .iter()
                .map(|v| wire(v, symbols))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            format!("[{kind:?}, [{values}]]")
        }
        Shape::Object(object) => {
            let fields = object
                .fields
                .iter()
                .map(|f| {
                    Ok(format!(
                        "[{}, {}, {}]",
                        render::string(&f.name),
                        wire(&f.shape, symbols)?,
                        f.presence == Presence::Optional
                    ))
                })
                .collect::<Result<Vec<_>, Error>>()?
                .join(", ");
            format!(
                "[\"object\", {}, [{fields}]]",
                object.unknown_fields == UnknownFields::Ignore
            )
        }
    })
}
