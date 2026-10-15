use spline_example_contract::{error_status, users};
use spline_openapi::OpenApi;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mode = match args.as_slice() {
        [mode] if mode == "--write" || mode == "--check" => mode,
        _ => return Err("usage: cargo run -p spline-example-contract --example contract-openapi -- --write|--check".into()),
    };
    // The workbench keeps its executable policy, demonstrating opaque status metadata.
    let binding = users::http::describe_with(&error_status)?;
    let source = OpenApi::new("Users API", "0.1.0")
        .export_contract(&binding)?
        .to_pretty_json()?;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("openapi.json");
    if mode == "--write" {
        std::fs::write(path, source)?;
    } else if std::fs::read_to_string(path)? != source {
        return Err("Users OpenAPI document is stale; regenerate with --write".into());
    }
    println!(
        "Users OpenAPI {}",
        if mode == "--write" {
            "generated"
        } else {
            "is current"
        }
    );
    Ok(())
}
