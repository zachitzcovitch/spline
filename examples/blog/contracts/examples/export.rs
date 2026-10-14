use spline_example_blog_contracts::api;
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

    let source = Typescript::new()
        .export_http_application(
            &api::http::describe_with(&spline::http::DeclaredErrors)?,
            "Blog",
        )?
        .source;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../typescript/generated.ts");
    if mode == "--write" {
        std::fs::write(path, source)?;
        println!("Blog TypeScript client generated");
    } else {
        if std::fs::read_to_string(path)? != source {
            return Err("Blog TypeScript client is stale; run cargo run --locked -p spline-example-blog-contracts --example blog-export -- --write".into());
        }
        println!("Blog TypeScript client is current");
    }
    Ok(())
}
