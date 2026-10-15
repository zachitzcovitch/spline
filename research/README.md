# Research history

These notes record the decisions and experiments that led to the implementation.
The runtime, derive, exporter, and their tests have now moved into the root
workspace. Use the [root README](../README.md) and
[workspace/contract guide](../docs/workspace-contracts.md) for current commands.
The [HTTP guide](../docs/http-binding.md) describes the executable server and both
clients built on that foundation.

## Notes

- [Schema research](schema.md): source review, directional shapes, and Serde behavior.
- [Serde internals reuse](serde-internals-reuse.md): private-adapter evaluation.
- [Schema registry](schema-registry.md): graph, identity, field rules, and recursion.
- [Schema derive](schema-derive.md): compiler-side lowering and support boundaries.
- [TypeScript export](typescript-exporter.md): naming, wire-type mapping, and compiler checks.
- [OpenAPI export](openapi.md): Utoipa evaluation, document representation, and independent validation.

## Implementation migration

| Former prototype | Current code and evidence |
| --- | --- |
| `schema-registry-probe` | [`crates/spline-schema`](../crates/spline-schema), including its 20 conformance tests. |
| `schema-derive-probe` | [`crates/spline-macros`](../crates/spline-macros) and [`crates/spline`](../crates/spline), including derive behavior tests and diagnostic fixtures. |
| `typescript-exporter-probe` | [`crates/spline-typescript`](../crates/spline-typescript), including the generator, Rust tests, TypeScript fixtures, and checked-in artifacts. |

The relocated prototypes' old manifests and lockfiles were replaced by root
workspace manifests and lockfiles. Historical notes retain their original
verification evidence; commands referring to the former manifests are historical.

The standalone probe packages, duplicate lockfiles, and placeholder prototype
directories have been removed. Their useful evidence is now maintained in the
root workspace and exercised by CI:

| Former experiment | Maintained evidence |
| --- | --- |
| Serde attribute inspection | [`spline-schema/tests/support/parser`](../crates/spline-schema/tests/support/parser), a test-only adapter and owned inspection model. |
| Parser vs. real Serde behavior | [`serde_inspection.rs`](../crates/spline-schema/tests/serde_inspection.rs), 19 comparison tests, and [`tests/ui`](../crates/spline-schema/tests/ui), three compiler-failure fixtures. |
| Serde JSON wire behavior | [`serde_behavior.rs`](../crates/spline-schema/tests/serde_behavior.rs), 10 direct Serde checks using the current workspace versions. |
| JavaScript integer precision | [`json-numbers.mjs`](../crates/spline-typescript/typescript/json-numbers.mjs), included in `npm run check`. |

```sh
cargo test --locked -p spline-schema
npm run check
```

These notes remain as design history: their dates, proposed interfaces, benchmark
limitations, and original tool versions describe the evaluated milestones. Use
the linked current guides for supported APIs and the [roadmap](../ROADMAP.md) for
future work. The production compiler adapter lives in
[`spline-macros/src/adapter.rs`](../crates/spline-macros/src/adapter.rs).
