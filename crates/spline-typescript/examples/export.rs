#[path = "../fixtures/api.rs"]
mod api;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mode = match args.as_slice() {
        [mode] if mode == "--check" || mode == "--write" => mode,
        _ => return Err("usage: cargo run --example export -- --check|--write".into()),
    };
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("typescript");
    let artifact = api::artifact(false)?;
    for (name, expected) in [
        ("generated.ts", artifact.source),
        ("wire-samples.ts", api::samples()?),
    ] {
        let path = directory.join(name);
        if mode == "--check" {
            let actual = std::fs::read_to_string(&path)?;
            if actual != expected {
                return Err(format!("{} is stale; regenerate with --write", path.display()).into());
            }
        } else {
            std::fs::write(&path, expected)?;
        }
    }
    println!(
        "TypeScript artifacts {}",
        if mode == "--check" {
            "are current"
        } else {
            "generated"
        }
    );
    Ok(())
}
