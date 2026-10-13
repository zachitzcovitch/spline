use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use spline_contract::{http::HttpMethod, ContractDescription};
use spline_schema::{DefinitionId, Presence, Shape, UnknownFields};

use crate::{contract, render, Artifact, Error, Typescript};

impl Typescript {
    pub fn export_http_client(
        &self,
        contract: &ContractDescription,
        name: &str,
    ) -> Result<Artifact, Error> {
        let mut artifact = self.export_contract(contract, name)?;
        let occupied: BTreeSet<_> = artifact
            .symbols
            .values()
            .cloned()
            .chain(contract.operations().values().flat_map(|op| {
                ["Input", "Output", "Error"].map(|suffix| {
                    format!(
                        "{}_{}_{suffix}",
                        contract.namespace().replace('.', "_"),
                        op.name
                    )
                })
            }))
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
        for (id, definition) in contract.schemas().definitions() {
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
        for operation in contract.operations().values() {
            let plan = operation
                .http
                .as_ref()
                .ok_or_else(|| Error::MissingHttpBinding(operation.id.clone()))?;
            let method = match plan.binding.method {
                HttpMethod::Get => "GET",
                HttpMethod::Post => "POST",
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
            let stem = format!(
                "{}_{}",
                contract.namespace().replace('.', "_"),
                operation.name
            );
            writeln!(methods, "    [{}]: ({parameters}) => call<{stem}_Output, {stem}_Error>({{\n      method: {method:?}, path: {}, body: {body}, status: {}, paths: [{paths}], queries: [{queries}],\n      input: {}, output: {}, error: {},\n    }}, {{ {fields} }}),",
                render::string(&operation.name), render::string(&plan.binding.path), plan.binding.success_status,
                wire(&operation.input, &artifact.symbols)?, wire(&operation.output, &artifact.symbols)?, wire(&operation.error, &artifact.symbols)?,
            ).unwrap();
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
