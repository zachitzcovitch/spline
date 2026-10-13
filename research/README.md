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

## Implementation migration

| Former prototype | Current code and evidence |
| --- | --- |
| `schema-registry-probe` | [`crates/spline-schema`](../crates/spline-schema), including its 20 conformance tests. |
| `schema-derive-probe` | [`crates/spline-macros`](../crates/spline-macros) and [`crates/spline`](../crates/spline), including derive behavior tests and diagnostic fixtures. |
| `typescript-exporter-probe` | [`crates/spline-typescript`](../crates/spline-typescript), including the generator, Rust tests, TypeScript fixtures, and checked-in artifacts. |

The relocated prototypes' old manifests and lockfiles were replaced by root
workspace manifests and lockfiles. Historical notes retain their original
verification evidence; commands referring to the former manifests are historical.

The standalone [Serde wire probe](serde-wire-probe/Cargo.toml) and
[Serde internals probe](serde-internals-probe/README.md) remain here with their
original dependencies and tests. They are excluded from the root workspace;
the schema crate uses the latter only as a dev dependency for comparison tests.
