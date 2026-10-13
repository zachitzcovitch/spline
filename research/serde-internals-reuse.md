# Evaluating serde_derive_internals reuse

Evaluated: 2026-10-12.

**Workspace update:** the inspection adapter and its tests now live under
[`spline-schema/tests`](../crates/spline-schema/tests). The old standalone package
and lockfile were removed; the evaluation below records the original findings.

## Recommendation

**Reuse `serde_derive_internals` behind a private adapter in the next Spline schema
prototype.** It provides useful, tested interpretation of Serde attributes and
validation rules. Spline should own the schema graph, supported representation
policy, type registration, and exporters.

The [test-only inspection adapter](../crates/spline-schema/tests/support/parser) demonstrates that this
boundary is practical. All references to Serde's internal AST and Syn stay in
`adapter.rs`; the returned metadata consists of owned Spline-defined types.

The recommended production dependency is an exact pin to the evaluated release,
with upgrades treated as explicit adapter changes. This is a recommendation for
the next prototype, not a declaration that the experiment is a finished schema
engine or supports every Serde attribute.

## What was evaluated

| Component | Executed version |
| --- | --- |
| `serde_derive_internals` | 0.30.0 |
| Syn | 3.0.3 |
| Serde and its derives | 1.0.229 |
| serde_json | 1.0.154 |
| trybuild | 1.0.122, resolved in the lockfile |
| Rust toolchain | 1.95.0-nightly (859951e3c, 2026-02-24) |

The previous wire-behavior probe retains its original pinned dependencies. This
experiment evaluates the newer parser and Serde pair independently.

The test fixtures define a Rust type once, then use that same declaration in two
ways: compile its real Serde derives, and inspect its syntax through the adapter.
This lets tests compare parser metadata against actual JSON behavior.

The prototype intentionally returns inspection data. Rust type and path strings
are diagnostic representations, not resolved types or schema identities. Only the
metadata families exercised by the experiment are projected into its model.

## Results

The full run passed **19 compatibility/adapter tests and three compiler-failure
fixtures**. Formatting and Clippy with warnings denied also passed.

| Area | Evidence |
| --- | --- |
| Naming | Directional `rename_all`, field overrides, raw identifiers, aliases, and variant-field precedence match actual Serde behavior. |
| Presence-related facts | Defaults, conditional omission, and skipped fields are exposed separately. A plain Option can accept a missing field despite having no recorded default. |
| Enum representations | External, internal, adjacent, and untagged representations are identifiable; emitted examples match their metadata. |
| Directional validation | Transparent-field selection differs by direction, and a Serialize-only transparent type can fail Deserialize validation. |
| Custom encoding | `with` exposes separate codec paths; `try_from` and `into` expose directional conversions. The actual wire type still depends on the codec. |
| Synthesized metadata | Borrowing a `Cow<str>` introduces a Serde-generated deserializer which the adapter recognizes and normalizes. |
| Other representation facts | Flattening and unknown-field policy are visible; the experiment verifies examples through Serde. |
| Rust syntax | Generic parameters, where clauses, explicit Serde bounds, and documentation can be retained without resolving referenced types. |
| Diagnostics | Multiple attribute errors are returned together with source locations. Syntax errors and unsupported unions return errors without panicking. |
| Dependency boundary | Returned metadata is owned, `Send + Sync + 'static`, and usable after the source string and parser AST are dropped. |

### 1. Direction is needed during parsing and validation

`Container::from_ast` takes `Derive::Serialize` or `Derive::Deserialize`. The choice
affects validation and normalized metadata, not just the eventual exported name.

For example, this type selects a different transparent member in each direction:

```rust
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
struct Split {
    #[serde(skip_deserializing)]
    emitted: String,
    #[serde(skip_serializing)]
    accepted: u32,
}
```

The test confirms string serialization and numeric deserialization. Separately, a
transparent newtype with a defaulted field can pass Serialize validation and fail
Deserialize validation.

**Implication:** inspect each required direction using the matching mode. Parsing
everything once as Deserialize and then switching field names is insufficient.

### 2. Attribute facts are not a complete wire schema

The helper reports `Default::None` for a plain `Option<String>` field, yet Serde
accepts that field being absent. The adapter deliberately calls that state
`DefaultRule::Unspecified` rather than `Required`.

Similarly, `#[serde(with = "decimal")]` on a `u64` field gives us the paths
`decimal::serialize` and `decimal::deserialize`. The probe proves that those
functions emit and accept a string, but the parser cannot infer that from their
names or the field type.

**Implication:** Spline must combine attribute facts with schema implementations
for referenced types and explicit descriptions for custom codecs. The helper
does not resolve Rust types, inspect function bodies, or decide field presence on
its own. Serde trait bounds are not automatically the right bounds for a future
`Schema` implementation either.

### 3. Serde normalizes and synthesizes metadata

The helper applies naming-rule precedence and expands `with` into separate
serialization/deserialization paths. It can also supply defaults for skipped
fields and synthesize a borrowing deserializer for a `Cow` field.

In 0.30.0, the caller supplies a private-module identifier used by synthesized
paths. The experiment passes a local sentinel and translates the known borrowing
helper into an owned `Codec` variant. This normalization is source-sensitive and
belongs inside the version-pinned adapter. It does not emit calls to Serde's
private runtime modules.

**Implication:** the result is a Serde-oriented intermediate representation,
including some implementation details. It should be translated into Spline's
own vocabulary before other code consumes it.

### 4. Validation happens at more than one stage

The three compiler-failure fixtures establish a useful distinction:

| Fixture | Adapter result | Real compiler result |
| --- | --- | --- |
| Identical adjacent tag/content names | Returns the Serde validation error. | Serde derive rejects it with the corresponding message. |
| Borrowed field using a lifetime parameter named `'de` | Parsing succeeds. | A later Serde derive check rejects it. |
| Nonexistent `deserialize_with` function | Parsing succeeds and retains the path. | Rust name resolution rejects the generated reference. |

The helper also deliberately ignores attributes outside Serde's namespace. A
misspelled setting inside `#[spline(...)]` is Spline's responsibility to validate.

**Implication:** keep actual compile tests alongside parser tests. Spline also
needs its own support pass for representations such as flattening, custom codecs,
and untagged variants before claiming that an operation is exportable.

### 5. Error handling has a small but important lifecycle rule

`Ctxt::check()` consumes the accumulated diagnostics. Dropping the context without
calling it panics. An unsupported Rust union returns `None` from `from_ast`, so
handling that `None` with an early return before checking the context would be a
bug.

The adapter consumes the context first, then handles the optional AST. Tests
exercise both syntax errors and the union path. Production macro code should keep
real token spans through validation; the experiment converts them to positions
for assertions against source strings.

## How unstable is the dependency in practice?

A source/API comparison between releases 0.29.1 and 0.30.0 found concrete changes:

| Surface | 0.29.1 | 0.30.0 |
| --- | --- | --- |
| Syn dependency | Syn 2 | Syn 3 |
| `Container::from_ast` | Context, AST, direction | Also takes a private-module identifier. |
| Name representation | String-oriented `attr::Name` | `name::MultiName` and span-carrying `Name` values. |
| Alias collection | String values | Span-carrying name values. |
| Aggregate flatten helpers | Container `has_flatten`/`mark_has_flatten` | Removed; per-field flatten metadata remains. |

This comparison is based on published source and APIs. The executable adapter is
tested against **0.30.0 only**; it is not a dual-version compatibility layer.

These changes are manageable at a narrow adapter boundary, but they rule out
assuming the internal AST is a stable public dependency. Reuse saves implementing
and maintaining Serde's attribute interpretation; it trades that work for tracking
a smaller integration surface.

The inspected normal dependency tree consists of the parser, Syn, `quote`,
`proc-macro2`, and their shared identifier dependency. Serde, serde_json, and
trybuild are dev dependencies for this experiment. In Spline's eventual layout,
the parser should be a macro-side dependency. No compile-time performance or MSRV
compatibility beyond the executed toolchain is established by these tests.

## Proposed production boundary

```text
Spline derive crate
  rustc's input tokens
    -> Syn AST
    -> private serde_derive_internals adapter, per required direction
    -> Spline-owned macro description + Spline support/override validation
    -> generated Rust Schema implementations

Spline schema crate
  Schema trait + directional registry + normalized wire-shape graph

Exporters
  consume Spline's graph
```

The experiment's string-based `inspect` function is a convenient test interface.
A production macro should retain Rust types and spans on its compiler side,
emitting references to other types' schema implementations for rustc to resolve.
The runtime graph and exporter interfaces should contain neither Syn types nor
Serde's internal AST.

Recommended dependency and upgrade policy:

1. Pin `serde_derive_internals` exactly in the macro-side manifest. The experiment
   also pins Syn to make the evaluation reproducible.
2. Keep upstream adaptation in one private module. Do not expose its types through
   Spline's public API or emit references to Serde's private modules.
3. Before upgrading, review the upstream AST/check changes, update the adapter,
   and run both behavior comparisons and compiler fixtures.
4. Define and test a supported public Serde version window separately. Pinning the
   parser does not pin downstream applications' Serde versions; a library's
   lockfile does not establish that compatibility for its users.
5. Review diagnostic snapshots when the compiler changes. Preserve meaningful
   errors rather than assuming exact compiler text is stable forever.

## Reproduce the evaluation

From the repository root:

```sh
cargo test --locked -p spline-schema --test serde_inspection --test ui
cargo fmt --all -- --check
cargo clippy --locked -p spline-schema --all-targets -- -D warnings
```

The expected result is 19 passing compatibility tests plus a passing trybuild test
covering three compiler-failure fixtures. The inspected dependency versions are
recorded in the root lockfile.

## Next design step

The reuse evaluation supports moving on to **the `Schema` trait, registry, and
directional JSON shape model**. Use the existing struct, tagged enum, and
custom-codec cases, then add concrete generic instantiations and recursive types.

The next prototype should connect type-owned schema descriptions to the adapter's
attribute facts. In particular, it needs an explicit representation for field
presence and a way to require custom-codec descriptions. The choice of shared
versus adapted Rust client DTOs remains a separate contract-design question.

Follow-up: the [schema/registry prototype](schema-registry.md) now implements this
boundary, including a test-only bridge from the adapter's facts to typed schema
calls. It records the resulting API proposal and verified cases.

## References

- [Inspection adapter](../crates/spline-schema/tests/support/parser) and [comparison tests](../crates/spline-schema/tests/serde_inspection.rs).
- [0.30.0 container API](https://docs.rs/serde_derive_internals/0.30.0/serde_derive_internals/ast/struct.Container.html).
- [0.29.1 container API](https://docs.rs/serde_derive_internals/0.29.1/serde_derive_internals/ast/struct.Container.html).
- [0.30.0 published source](https://docs.rs/crate/serde_derive_internals/0.30.0/source/).
- [0.29.1 published source](https://docs.rs/crate/serde_derive_internals/0.29.1/source/).
- [Original schema research](schema.md), including the Schemars precedent for
  reusing Serde's attribute machinery.
