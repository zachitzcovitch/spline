# Contributing to Spline

Spline is an experimental workspace. The implementation, conformance tests, and
design notes evolve together; packages are currently unpublished.

## Development setup

Install [rustup](https://rustup.rs/) and Node.js. The repository pins the evaluated
Rust toolchain in `rust-toolchain.toml` and Node in `.nvmrc`. Compiler-diagnostic
snapshots are tied to that Rust version, and the HTTP tests use Node's built-in
TypeScript stripping.

With nvm available, run from the repository root:

```sh
nvm install
nvm use
rustup show
npm ci --ignore-scripts --no-audit --no-fund
```

Other Node version managers work too; use the version recorded in `.nvmrc`.
Cargo and npm lockfiles are checked in so local and CI dependencies agree.

## Verify a change

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo check --locked -p spline --no-default-features
cargo test --locked --workspace
cargo run --locked -p spline-typescript --example export -- --check
cargo run --locked -p spline-example-contract -- --check
npm run check
```

[CI](.github/workflows/ci.yml) runs the same checks. The HTTP integration suite
starts ephemeral loopback servers and launches the Rust and Node clients. Node
and loopback networking must be available for that suite.

For focused iteration, use `cargo test -p <crate>` or a specific integration test,
such as `cargo test -p spline-example-contract --test http`.

## Generated artifacts and diagnostics

Rust fixtures and contracts are the source of truth for checked-in TypeScript.
After an intentional change to those inputs or the exporter, regenerate:

```sh
cargo run --locked -p spline-typescript --example export -- --write
cargo run --locked -p spline-example-contract -- --write
```

Review the generated diff and rerun the relevant Rust and TypeScript checks.
`--check` validates freshness without rewriting files. Both generated declarations
and wire samples are covered by tests, and the TypeScript suites include negative
assertions that fail if an invalid call unexpectedly becomes accepted.

Compiler-failure tests live under `crates/spline/tests/ui/`. When a diagnostic
intentionally changes, review the proposed `.stderr` output in `wip/` before
updating the checked-in expectation. Generated diagnostic snapshots are part of
the user-facing macro behavior.

## Where changes belong

- `spline-schema`: directional shapes, registration, and field semantics.
- `spline-contract`: operation identities, argument order, and binding metadata.
- `spline-macros`: schema/contract authoring and generated Rust; Serde's internal
  API stays behind the private adapter.
- `spline`: application-facing re-exports and macro conformance tests.
- `spline-http`: HTTP encoding, Axum/reqwest integration, and client errors.
- `spline-typescript`: deterministic type/client export and TypeScript fixtures.
- `examples/contract`: runnable server and clients, including end-to-end tests.

Update the relevant guide in `docs/` when changing public behavior. Use the
[research index](research/README.md) to understand the decisions behind the
implementation. The excluded standalone research packages retain historical
experiments; current development and CI target the root workspace.

For behavior changes, prefer evidence at the boundary being changed: real Serde
encoding/decoding, reviewed macro diagnostics, TypeScript compiler assertions, or
both generated clients calling the same HTTP server.
