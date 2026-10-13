fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mode = match args.as_slice() {
        [mode] if mode == "--check" || mode == "--write" => mode,
        _ => return Err("usage: cargo run -p spline-example-contract -- --write|--check".into()),
    };
    let source = spline_example_contract::typescript()?;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("typescript/generated.ts");
    if mode == "--check" {
        if std::fs::read_to_string(&path)? != source {
            return Err("contract TypeScript is stale; regenerate with --write".into());
        }
    } else {
        std::fs::write(path, source)?;
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
