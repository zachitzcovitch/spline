# TypeScript exporter prototype — promoted

The implementation now lives in [`crates/spline-typescript`](../../crates/spline-typescript).
It consumes the schema runtime's graph and emits a deterministic TypeScript module.
See the [design note](../typescript-exporter.md) for the mapping, naming rules,
verification evidence, and remaining decisions.

## Artifacts

- [generated.ts](../../crates/spline-typescript/typescript/generated.ts): generated input/output wire types.
- [wire-samples.ts](../../crates/spline-typescript/typescript/wire-samples.ts): real Serde values checked with `satisfies`.
- [assertions.ts](../../crates/spline-typescript/typescript/assertions.ts): positive assertions and 25 expected-error cases.

## API

```rust
let artifact = Typescript::new().export(&graph, &roots)?;
```

`Artifact` contains source text and the definition-to-symbol map. `Root` supplies
a symbol and direction for an inline shape. `Typescript::name(id, symbol)` overrides
a named definition's default symbol. The exporter performs no filesystem I/O;
the example below handles artifact generation and freshness checking.

The normal dependency tree contains the schema and contract-metadata crates.
The Rust fixtures use the real derive macro and Serde as dev dependencies.

## Run from the repository root

```sh
# Regenerate after intentionally changing the Rust fixtures or renderer.
cargo run --locked -p spline-typescript --example export -- --write

# Checks suitable for CI.
cargo run --locked -p spline-typescript --example export -- --check
cargo test --locked -p spline-typescript
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
npm ci --ignore-scripts --no-audit --no-fund
npm run check
```

The original 14 exporter tests moved with the implementation; contract-export
tests now run alongside them. TypeScript 5.9.3 is pinned at the workspace root.
See the [workspace guide](../../docs/workspace-contracts.md) for current evidence.
