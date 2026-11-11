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
| `schema-registry-probe` | [`crates/splinekit-schema`](../crates/splinekit-schema), including its 20 conformance tests. |
| `schema-derive-probe` | [`crates/splinekit-macros`](../crates/splinekit-macros) and [`crates/splinekit`](../crates/splinekit), including derive behavior tests and diagnostic fixtures. |
| `typescript-exporter-probe` | [`crates/splinekit-typescript`](../crates/splinekit-typescript), including the generator, Rust tests, TypeScript fixtures, and checked-in artifacts. |

The relocated prototypes' old manifests and lockfiles were replaced by root
workspace manifests and lockfiles. Historical notes retain their original
verification evidence; commands referring to the former manifests are historical.

The standalone probe packages, duplicate lockfiles, and placeholder prototype
directories have been removed. Their useful evidence is now maintained in the
root workspace and exercised by CI:

| Former experiment | Maintained evidence |
| --- | --- |
| Serde attribute inspection | [`splinekit-schema/tests/support/parser`](../crates/splinekit-schema/tests/support/parser), a test-only adapter and owned inspection model. |
| Parser vs. real Serde behavior | [`serde_inspection.rs`](../crates/splinekit-schema/tests/serde_inspection.rs), 19 comparison tests, and [`tests/ui`](../crates/splinekit-schema/tests/ui), three compiler-failure fixtures. |
| Serde JSON wire behavior | [`serde_behavior.rs`](../crates/splinekit-schema/tests/serde_behavior.rs), 10 direct Serde checks using the current workspace versions. |
| JavaScript integer precision | [`json-numbers.mjs`](../crates/splinekit-typescript/typescript/json-numbers.mjs), included in `npm run check`. |

```sh
cargo test --locked -p splinekit-schema
npm run check
```

These notes remain as design history: their dates, proposed interfaces, benchmark
limitations, and original tool versions describe the evaluated milestones. Use
the linked current guides for supported APIs and the [roadmap](../ROADMAP.md) for
future work. The production compiler adapter lives in
[`splinekit-macros/src/adapter.rs`](../crates/splinekit-macros/src/adapter.rs).
