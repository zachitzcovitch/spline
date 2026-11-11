# Contributing to Splinekit

Splinekit is an experimental workspace. The implementation, conformance tests, and
design notes evolve together; packages are currently unpublished.

## Ways to contribute

- Try the [blog tutorial](examples/blog/README.md) and report confusing setup or APIs.
- Open a [bug report](https://github.com/zachitzcovitch/spline/issues/new/choose)
  with a small reproduction, expected behavior, commit, and toolchain versions.
- Use the [roadmap](ROADMAP.md) for upcoming work. For a larger feature, describe
  the use case and proposed contract/client experience in an issue first.
- Send focused pull requests explaining the change and checks run. Update the
  relevant guide and the Unreleased section of [CHANGELOG.md](CHANGELOG.md) for
  user-visible behavior changes.

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
cargo check --locked -p splinekit --no-default-features
npm run check:neutral
cargo run --locked -p splinekit --no-default-features --example neutral
cargo test --locked --workspace
cargo run --locked -p splinekit-typescript --example export -- --check
cargo run --locked -p splinekit-example-contract -- --check
cargo run --locked -p splinekit-example-blog-contracts --example blog-export -- --check
cargo run --locked -p splinekit-example-contract --example contract-openapi -- --check
npm run check
npm run check:openapi
npm run test:blog
cargo bench --locked -p splinekit-benchmarks -- --test
npm run bench:typescript -- --check
```

[CI](.github/workflows/ci.yml) runs the same checks. The HTTP integration suite
starts ephemeral loopback servers and launches the Rust and Node clients. Node
and loopback networking must be available for that suite.

For focused iteration, use `cargo test -p <crate>` or a specific integration test,
such as `cargo test -p splinekit-example-contract --test http`.

For performance work, see [benchmarks/README.md](benchmarks/README.md). Keep fixture
correctness checks separate from timing comparisons. Include the commit, toolchain,
hardware, workload, and commands when reporting results; retain raw local baseline
data under the ignored Cargo target directory.

## Generated artifacts and diagnostics

Rust fixtures and contracts are the source of truth for checked-in TypeScript and OpenAPI.
After an intentional change to those inputs or the exporter, regenerate:

```sh
cargo run --locked -p splinekit-typescript --example export -- --write
cargo run --locked -p splinekit-example-contract -- --write
cargo run --locked -p splinekit-example-blog-contracts --example blog-export -- --write
cargo run --locked -p splinekit-example-contract --example contract-openapi -- --write
```

Review the generated diff and rerun the relevant Rust and TypeScript checks.
`--check` validates freshness without rewriting files. Both generated declarations
and wire samples are covered by tests, and the TypeScript suites include negative
assertions that fail if an invalid call unexpectedly becomes accepted.

Compiler-failure tests live under `crates/splinekit/tests/ui/`, its HTTP-specific
`ui-http/` directory, and `crates/splinekit-schema/tests/ui/`. When a diagnostic
intentionally changes, review the proposed `.stderr` output in `wip/` before
updating the checked-in expectation. Generated diagnostic snapshots are part of
the user-facing macro behavior.

Contract-evolution fixtures live in `examples/contract/fixtures/evolution.rs`.
Keep the older `v1` contract fixed when adding newer-server scenarios. The contract
generator also maintains its TypeScript client, and the `evolution` integration
suite compares Rust and TypeScript behavior against the same versioned servers.

Application-composition fixtures live in `examples/contract/fixtures/application.rs`.
The same generator maintains their grouped TypeScript client. The `application`
integration suite exercises both clients with shared state/authentication; core
builder and exporter tests cover transactional registration and naming/route conflicts.

The [blog tutorial](examples/blog/README.md) has separate contract, server, and
Rust client crates plus a TypeScript package. Keep its walkthrough focused. Its
exporter is a Cargo example with development-only dependencies in the contracts
crate. `npm run test:blog` builds both application packages and uses Cargo's JSON
artifact paths to launch the actual server and client entry points. This check
runs separately from `cargo test`, avoiding test-only dependencies between the
application packages. Its TypeScript workspace checks the handwritten client
along with the generated artifact.
Its TypeScript language-service check also verifies signature help, narrowed hovers,
and DTO property completion. Preserve that editor-facing surface when changing
names or generated signatures; see [docs/typescript.md](docs/typescript.md).

Declared-error behavior is covered by `crates/splinekit/tests/http_errors.rs` and the
HTTP compiler fixtures. Keep real statuses and serialized response metadata aligned;
see [docs/http-errors.md](docs/http-errors.md) for derived defaults and custom policies.

OpenAPI conformance lives in `crates/splinekit-openapi/tests/`. Its Rust tests run the
independent Node validators, so install root npm dependencies before running them.
The blog exporter checks both its TypeScript and OpenAPI artifacts; the workbench's
OpenAPI generator is a development-only Cargo example. `npm run check:openapi`
validates both documents, and the live blog workflow checks real response bodies
against the selected schemas. See [docs/openapi.md](docs/openapi.md).

## Where changes belong

- `splinekit-schema`: directional shapes, registration, and field semantics.
- `splinekit-contract`: neutral operation identities, argument order, and composition.
- `splinekit-http-contract`: HTTP binding descriptions and planning/route validation.
- `splinekit-runtime`: neutral definitions/bundles, typed Tower invocation, and in-process extensions.
- `splinekit-macros`: schema/contract authoring and generated Rust; Serde's internal
  API stays behind the private adapter.
- `splinekit`: application-facing re-exports and macro conformance tests.
- `splinekit-http`: HTTP extensions, encoding, Axum/reqwest integration, and client errors.
- `splinekit-typescript`: deterministic type/client export and TypeScript fixtures.
- `splinekit-openapi`: deterministic document/schema lowering and OpenAPI conformance.
- `examples/contract`: runnable server and clients, including end-to-end tests.
- `examples/blog`: guided Users/Posts tutorial and its end-to-end smoke test.
- `benchmarks`: Criterion metadata/export/HTTP benchmarks and a Node client harness.
- `scripts/check-neutral.mjs`: guards neutral execution and the exporter's runtime-free boundary.

Update the relevant guide in `docs/` when changing public behavior. Use the
[research index](research/README.md) to understand the decisions behind the
implementation. Research notes retain the historical rationale; the former probe
packages have been removed and their useful Serde checks now run in the root suite.

For behavior changes, prefer evidence at the boundary being changed: real Serde
encoding/decoding, reviewed macro diagnostics, TypeScript compiler assertions, or
both generated clients calling the same HTTP server.

## License

Unless you explicitly state otherwise, contributions intentionally submitted for
inclusion in Splinekit are licensed under both the [MIT license](LICENSE-MIT) and
[Apache License 2.0](LICENSE-APACHE), without additional terms or conditions.
