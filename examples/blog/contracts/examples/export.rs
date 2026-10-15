use spline_example_blog_contracts::api;
use spline_openapi::OpenApi;
use spline_typescript::Typescript;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mode = match args.as_slice() {
        [mode] if mode == "--write" || mode == "--check" => mode,
        _ => return Err(
            "usage: cargo run -p spline-example-blog-contracts --example blog-export -- --write|--check"
                .into(),
        ),
    };

    let binding = api::http::describe_with(&spline::http::DeclaredErrors)?;
    let typescript = Typescript::new()
        .export_http_application(&binding, "Blog")?
        .source;
    let openapi = OpenApi::new("Blog API", "0.1.0")
        .server("/api")
        .bearer_auth("bearerAuth")
        .export_application(&binding)?
        .to_pretty_json()?;
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for (name, source) in [
        ("typescript/generated.ts", typescript),
        ("openapi.json", openapi),
    ] {
        let path = directory.join(name);
        if mode == "--write" {
            std::fs::write(path, source)?;
        } else if std::fs::read_to_string(path)? != source {
            return Err(format!("Blog artifact {name} is stale; run cargo run --locked -p spline-example-blog-contracts --example blog-export -- --write").into());
        }
    }
    println!(
        "Blog artifacts {}",
        if mode == "--write" {
            "generated"
        } else {
            "are current"
        }
    );
    Ok(())
}
