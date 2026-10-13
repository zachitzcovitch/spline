# A narrow Schema derive

Implemented: 2026-10-12.

**Workspace update:** the implementation now lives in
[`spline-macros`](../crates/spline-macros) and [`spline`](../crates/spline), with all
derive tests migrated. The [workspace guide](../docs/workspace-contracts.md) has
current commands; the prototype commands below are historical.

## Result

The [derive workspace](schema-derive-probe/README.md) now generates the typed
descriptions previously written by hand in the [registry prototype](schema-registry.md).
Its facade re-exports the runtime API and `#[derive(Schema)]`.

The proc macro uses an exactly pinned `serde_derive_internals` 0.30.0 adapter,
retaining Syn types and spans on the compiler side. Only `adapter.rs` imports
Serde's internal AST. Generated descriptions call the runtime directly; they
neither invoke the parser nor reference Serde's private runtime modules.

This is implemented in an isolated research workspace. The syntax below works
there; it is still a candidate API for the eventual Spline crates.

## Authoring

```rust
#[derive(serde::Serialize, serde::Deserialize, Schema)]
#[spline(name = "api.User")]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct User {
    display_name: String,
    nickname: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, skip_serializing_if = "::core::option::Option::is_none")]
    bio: Option<String>,
}
```

This generates the existing `Schema` interface: `info()`, `describe()`, and the
missing-field query described below. Registration remains:

```rust
let input = registry.register::<User>(Direction::Deserialize)?;
let output = registry.register::<User>(Direction::Serialize)?;
```

### Spline attributes

All values in this first syntax are string literals. Type and crate-path values
are parsed as Rust syntax and resolved by rustc in the caller's scope.

| Attribute | Location | Meaning |
| --- | --- | --- |
| `name = "api.User"` | Container | Logical base name; schema-relevant generic arguments are appended structurally. |
| `crate = "renamed_runtime"` | Container | Path to the runtime facade, useful with renamed imports/dependencies. |
| `only = "serialize"` or `"deserialize"` | Container | Generate one direction; registration in the other returns an explicit error. Both are the default. |
| `schema = "WireType"` | Container or field | Shared explicit schema provider for present wire values. |
| `serialize_as = "OutputWire"` | Container or field | Provider for the serialization view. |
| `deserialize_as = "InputWire"` | Container or field | Provider for the deserialization view. |
| `missing = "accepted"` or `"rejected"` | Container | Explicit assertion about the type's missing-field deserialization behavior; used when that direction is enabled. |

`schema` conflicts with directional overrides. Duplicate settings, unknown
settings, invalid names, and unsupported locations produce compiler diagnostics.
Without `name`, the default combines the Rust module path and type identifier.
Explicit names are preferable when an exported contract must survive Rust moves
or renames. The prototype requires an explicit `crate` override for a renamed
facade; it does not automatically inspect Cargo dependency aliases.

### Custom codecs

```rust
#[serde(with = "decimal")]
#[spline(schema = "String")]
count: u64,
```

The paired Serde module must actually serialize and deserialize the declared wire
form. A field override removes the requirement for the original field type to
implement `Schema`; a foreign type or opaque generic value can therefore have a
separate schema provider.

Shared or directional container overrides describe an entire type and bypass its
ordinary representation lowering. They are an explicit escape hatch for forms
such as an untagged enum. Serde's own syntax/structural checks still run.

Overrides are author-supplied descriptions, not codecs or proofs of arbitrary
validation logic. For opaque descriptions, missing-field behavior defaults to
rejected unless explicitly asserted. A field-level custom deserializer still
requires a field default to permit omission, as established by the earlier probes.

A field override describes present values only, even when its provider is
`Option<T>`. It cannot establish that the original Rust type accepts a missing
field. Input omission for such overrides therefore requires a declared
field/container default. A regression test compares a nullable plain newtype
against Serde and guards against incorrectly making that field optional.

## Supported automatic lowering

| Area | First slice |
| --- | --- |
| Structs | Named structs, including empty ones; unit structs; ordinary single-field newtypes. |
| Transparent structs | Direction-specific member selection, including recursive wrappers. |
| External enums | Unit, newtype, and named-field variants. |
| Adjacent enums | Unit, native newtype, and named-field variants; direction-specific content presence. |
| Internal enums | Unit and named-field variants. |
| Field semantics | Renames, defaults, skips, conditional omission, custom-codec descriptions, and unknown-field policy. |
| Naming rules | Serde's directional renames and container/variant precedence, interpreted by the adapter. |
| Generics | Owned type parameters, with original where clauses retained and inferred schema-relevant bounds. |
| Recursion | Self-recursive and mutually recursive descriptions through registry references. |
| Documentation | Container and named-field comments are retained. Union-arm documentation is not yet a graph feature. |

General tuple layouts, automatic flattening, input aliases, untagged lowering,
internally tagged newtype payloads, lifetime parameters, and const parameters
receive diagnostics. Whole-type overrides can replace representation lowering;
lifetime and const parameters still require a manual implementation in this slice.
Unsupported leaf Rust types are also subject to ordinary trait checking and the
registry's existing JSON profile. For example, raw `u64` still produces a profile
error at registration rather than becoming a JavaScript number silently.

Only the selected directions contribute field calls and bounds. Truly skipped
fields and variants emit no references to their payload schemas. Input aliases
are rejected when an input schema is requested; serialization-only descriptions
can ignore those deserialization-only aliases.

## Important implementation decisions

### Typed generics and recursive bounds

The derive retains actual Syn types and emits calls such as
`cx.field::<Vec<T>>()` and `cx.schema::<Node<T>>()`. It does not resolve Rust types
from diagnostic strings.

Adding a predicate such as `Vec<Node<T>>: Schema` to the generated implementation
would create a recursive trait obligation. Instead, the derive adds `Schema`
bounds to schema-relevant type parameters, retains the original where clause,
and lets rustc resolve the emitted calls inside the implementation.

Parameters used only in skipped fields or replaced native fields get a `'static`
bound for registry identity, but no `Schema` bound and no logical-name argument.
The tests include a generic payload that implements neither Serde nor Schema,
handled entirely by its custom codec and schema override.

There is a deliberate naming limitation: two concrete instantiations differing
only in hidden parameters claim the same logical name. The registry reports a
collision if both are exported in the same direction. It does not silently merge
them or invent order-dependent names. Richer hidden-parameter identity and more
general associated-type bound inference remain future decisions; named wrappers
or manual implementations can supply explicit identities and bounds.

### Missing-field queries must not build names

Generating a transparent recursive type exposed a recursion path in the earlier
metadata interface. Consider a transparent wrapper around `Vec<Self>`: asking for
the child's full `info()` to learn missing-field behavior would construct the
child's generic name, which asks for `Self::info()` again before registration can
reserve the type.

The runtime now has a backwards-compatible optional hook:

```rust
fn missing_field() -> MissingField {
    Self::info().missing
}
```

Existing handwritten implementations keep working. Standard Option, Vec, map,
and Box implementations answer this query independently of generic-name
construction. Generated implementations override the hook and keep the result in
their `TypeInfo` synchronized. The two required methods remain `info` and
`describe`.

Tests verify a recursive transparent vector wrapper as well as plain/transparent
nullable newtypes. This refines the earlier API based on an executable use case.

### Predicate paths need a precision policy

A procedural macro cannot establish that an arbitrary function named
`Option::is_none` is the standard method. The test suite deliberately shadows that
name with a function that always returns false.

The derive therefore uses the typed `OptionSchema::Value` projection only for the
supported, fully qualified paths `::core::option::Option::is_none` and
`::std::option::Option::is_none`, without explicit path generic arguments.
Short or aliased predicate paths remain supported as conservative conditional
omission: the property may be absent, but its original value shape is retained.

This keeps nested Options correct. `Some(None)` still emits a present null even
when the outer None is omitted. Custom schema overrides describe the present
values explicitly rather than being automatically refined.

### Adjacent enum payload absence differs from struct fields

Serde accepts an omitted content property for adjacent unit variants and for
ordinary newtype payloads whose native type accepts a missing field. The derive
models that behavior in the input view. It also accepts explicit null content for
a unit variant on input, while describing the unit variant's emitted form without
that content property.

Serde's missing-content path for a newtype variant can bypass a field-level codec
and use the native payload type's missing-field behavior. Reusing the ordinary
struct-field rule would be incorrect. This slice diagnoses adjacent newtype
codecs/overrides in the deserialize view instead of guessing; use a manual input
schema or whole-type directional override for that case.

The implementation was checked against the published Serde 1.0.229
`de/enum_adjacently.rs` logic and executable JSON fixtures.

## Validation evidence

The derive workspace passes:

- **25 behavior tests:** generated struct/enum shapes compared with real Serde,
  generic identities, recursion, field pruning, codec overrides, runtime-path
  overrides, and omission/missing-field behavior.
- **11 compiler-failure fixtures:** unsupported forms, missing codec descriptions,
  duplicate names, malformed options, invalid logical names, and unsupported
  generic kinds, with reviewed source spans.
- Formatting and Clippy with warnings denied.

The modified runtime also passes its original **20 integration tests**, formatting,
and Clippy. Its normal dependency graph remains std-only. The compiler dependency
on Serde internals is isolated in the macro crate.

From the repository root:

```sh
cargo test --locked --manifest-path research/schema-derive-probe/Cargo.toml --workspace
cargo fmt --manifest-path research/schema-derive-probe/Cargo.toml --all -- --check
cargo clippy --locked --manifest-path research/schema-derive-probe/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --locked --manifest-path research/schema-registry-probe/Cargo.toml
```

Versions are recorded in the lockfiles: Serde 1.0.229, serde_json 1.0.154, Syn 3.0.3,
and serde_derive_internals 0.30.0. Tests and compiler snapshots were reviewed on
rustc 1.95.0-nightly (859951e3c, 2026-02-24). This is the evaluated toolchain, not a
declared minimum supported Rust version.

## Next step

Follow-up: the [TypeScript exporter](typescript-exporter.md) is now implemented,
with checked-in generated artifacts, real Serde samples, and strict compiler
assertions. Its design note records the naming and wire-type mapping decisions.

The schema path now runs from Rust derives to a validated directional graph.
A small **TypeScript exporter** can exercise that graph end to end: stable symbol
names, references and recursion, optional versus nullable fields, and generated
input/output types checked by TypeScript itself.

Promoting the runtime and macro into the eventual public crate layout can follow
once that exported artifact validates the authoring and naming choices. Contract
traits, operation clients, and HTTP bindings can then build on the same schema
foundation.
