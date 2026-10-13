# Schema derive prototype — promoted

The macro implementation now lives in [`crates/spline-macros`](../../crates/spline-macros),
with the facade and conformance tests in [`crates/spline`](../../crates/spline).
See the [design and support matrix](../schema-derive.md) for the decisions behind it.

The facade re-exports both the runtime's `Schema` trait and the derive macro:

```rust
use serde::{Deserialize, Serialize};
use spline::{Direction, Registry, Schema};

#[derive(Serialize, Deserialize, Schema)]
#[spline(name = "api.User")]
#[serde(rename_all = "camelCase")]
struct User {
    display_name: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, skip_serializing_if = "::core::option::Option::is_none")]
    bio: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = Registry::new();
    let _input = registry.register::<User>(Direction::Deserialize)?;
    let _output = registry.register::<User>(Direction::Serialize)?;
    let graph = registry.finish()?;
    println!("{graph:#?}");
    Ok(())
}
```

## Run

From the repository root:

```sh
cargo test --locked -p spline
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

The original 25 derive behavior tests and 11 diagnostic fixtures moved with the
implementation. Contract tests now run alongside them. The root lockfile records
the evaluated dependencies; see the [workspace guide](../../docs/workspace-contracts.md).
