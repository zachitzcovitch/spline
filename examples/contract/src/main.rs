// Shared contract revisions are also exercised by the integration test suite.
#[path = "../fixtures/evolution.rs"]
#[allow(dead_code)]
mod evolution;

#[path = "../fixtures/application.rs"]
#[allow(dead_code)]
mod application;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mode = match args.as_slice() {
        [mode] if mode == "--check" || mode == "--write" => mode,
        _ => return Err("usage: cargo run -p spline-example-contract -- --write|--check".into()),
    };
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("typescript");
    for (name, source) in [
        ("generated.ts", spline_example_contract::typescript()?),
        ("evolution-v1.generated.ts", evolution::typescript()?),
        ("application.generated.ts", application::typescript()?),
    ] {
        let path = directory.join(name);
        if mode == "--check" {
            if std::fs::read_to_string(&path)? != source {
                return Err(format!("{name} is stale; regenerate with --write").into());
            }
        } else {
            std::fs::write(path, source)?;
        }
    }
    println!(
        "Contract TypeScript {}",
        if mode == "--check" {
            "is current"
        } else {
            "generated"
        }
    );
    Ok(())
}
