# Schema trait and directional registry

Prototyped: 2026-10-12.

**Workspace update:** this code and its tests now live in
[`crates/spline-schema`](../crates/spline-schema). Use the
[root verification commands](../docs/workspace-contracts.md#commands); the prototype
manifest commands recorded below describe the original experiment.

## Result

The [registry implementation](../crates/spline-schema) implements a `Schema`
trait with two required methods, a directional registry, and a small JSON
wire-shape model. It also connects the [previous attribute adapter](serde-internals-reuse.md)
to typed field descriptions in its test fixtures.

The proposed responsibilities work together in the evaluated cases:

- Types declare their identity and wire shape.
- Field builders combine type-owned facts with direction-specific Serde facts.
- The registry handles references, recursion, collisions, and failure recovery.
- The finished graph is an owned, ordered data structure for exporters.

All 20 integration tests pass. The library has a **std-only normal dependency
graph**; Serde, serde_json, and the attribute-adapter experiment are dev
dependencies. Public API names and packaging remain design proposals.

Follow-up: the [derive prototype](schema-derive.md) now emits these descriptions.
It adds a defaulted `missing_field()` query to support transparent recursive types
without recursively constructing their names; the two required methods remain
`info()` and `describe()`.

## 1. The trait

The required interface is:

```rust
pub trait Schema: 'static {
    fn info() -> TypeInfo;
    fn describe(cx: &mut Context<'_>) -> Result<Shape>;
}
```

`info()` supplies facts needed before visiting children: a logical name, whether
the description is inline or named, ordinary deserialization missing-field
behavior, and documentation.

`describe()` constructs the wire shape for `cx.direction()`. It requests children
through `cx.schema::<T>()`, preserving direction and letting the registry reserve
definitions before descending into recursive types.

For example, the decimal-string ID fixture implements:

```rust
impl Schema for UserId {
    fn info() -> TypeInfo {
        TypeInfo::named("demo.UserId")
            .documented("An ID encoded as a decimal string.")
    }

    fn describe(_: &mut Context<'_>) -> Result<Shape> {
        Ok(Shape::String)
    }
}
```

Its separate Serde implementations really encode and decode a string. `Schema`
describes that encoding; it does not change it.

`Schema` has no Serde supertraits. This permits direction-specific DTOs and
schema-only providers for custom codecs. Future transport code must enforce the
appropriate `Serialize` and `Deserialize` bounds at its call sites.

The `'static` bound permits process-local `TypeId` registration for the current
owned-DTO examples. Borrowed DTO support and other identity mechanisms need a
separate decision.

The derive follow-up added an optional `missing_field()` method, defaulting to
`Self::info().missing`. Field builders now use that query. Built-in wrappers and
generated descriptions can answer it independently of name construction, while
the handwritten implementations in this experiment retain their existing behavior.

## 2. Registry usage and identity

Callers retain the returned root shapes with the finished graph:

```rust
let mut registry = Registry::new();
let input = registry.register::<User>(Direction::Deserialize)?;
let output = registry.register::<User>(Direction::Serialize)?;
let graph = registry.finish()?;
```

Three naming/identity concerns are distinct:

| Concern | Representation |
| --- | --- |
| Rust identity | Private `(TypeId, Direction)` registration key. |
| Logical schema identity | `(Name, Direction)` in graph references. |
| Exporter spelling | A future exporter assigns target-language symbols. |

`Name` contains a namespace-qualified path and structured generic arguments:

```rust
TypeInfo::named(Name::applied("demo.Page", vec![T::info().name]))
```

This distinguishes `demo.Page<demo.User>` from `demo.Page<demo.Post>`. Input and
output definitions remain separate even when their shapes happen to match. The
prototype does not perform structural-equivalence merging.

Logical paths currently use dot-separated ASCII identifier segments. Generic
arguments are structured rather than embedded in the path string. Field names
remain arbitrary JSON property names. Final derive naming/override syntax is open.

Different Rust types claiming the same named definition in the same direction
produce a collision error, rather than traversal-order suffixes. Rust's
`type_name` is used only in diagnostics. Neither it nor `TypeId` is stored in the
finished graph.

## 3. Wire shapes

The prototype vocabulary is:

```text
Shape
  null | boolean | string | bounded integer | string literal
  array(element) | tuple(elements) | string-keyed map(value)
  object(fields, unknown-field policy)
  union(alternatives)
  reference(logical definition + direction)

Field
  wire name
  value shape
  presence: required | optional
  documentation
```

Tagged enums become ordinary object alternatives with literal tags. The
`ApiError` fixture verifies this against actual Serde output.

This profile is JSON-specific and fixed for a registry. Additional transports can
reuse it. Another codec would need an explicit profile design and profile-aware
caching, rather than reinterpretation of existing definitions.

The initial numeric slice includes signed/unsigned 8-, 16-, and 32-bit integers.
Raw larger integers, platform-sized integers, and floats return a profile error.
A real string codec is a tested way to represent a large integer. Integer widths
survive in the graph, although TypeScript's eventual `number` spelling cannot
itself enforce their ranges.

## 4. Field presence combines attributes and type facts

`FieldRules` records whether the field is skipped in the selected direction,
whether a deserialization default exists, its serialization omission policy, and
whether a field-level custom codec needs an explicit description.

The referenced type supplies ordinary missing-field behavior through `TypeInfo`.
This is independent of nullability:

| Situation | Field description |
| --- | --- |
| Skipped in this direction | No property is included. |
| Input with a field/container default | Property may be absent. |
| Ordinary input whose type accepts missing fields | Property may be absent. |
| Field-level custom input deserializer, without a default | Property is required, even with a nullable wire type. |
| Output without an omission predicate | Property is required. |
| Output with an omission predicate | Property is optional; value refinement needs separate evidence. |

Defaults are presence facts; schema export does not require executing application
default functions. The tested `User` views illustrate the resulting differences:

| Field | Accepted input | Emitted output |
| --- | --- | --- |
| `nickname: Option<String>` | Optional, nullable string | Required, nullable string |
| Defaulted `tags: Vec<String>` | Optional array | Required array |
| `bio` with `Option::is_none` omission | Optional, nullable string | Optional, non-null string |

The fixture uses a Rust alias for `Option<String>`. Field builders work through
trait resolution; the adapter bridge never guesses from type-name strings.

### Nullable does not imply that a field may be missing

A plain newtype around `Option<String>`, a Serde-transparent newtype around that
same type, and `Box<Option<String>>` all have nullable value representations.
Nevertheless, the plain newtype's field is required, while the transparent
newtype and boxed Option accept a missing field. Tests compare both the schemas
and actual Serde behavior.

### Omitting None needs a typed projection

The companion trait is:

```rust
pub trait OptionSchema: Schema {
    type Value: Schema;
}
```

`Context::option_field::<T>()` uses that associated type for a field serialized
only when its outer Option is `Some`. It does not erase all null alternatives.
For `Option<Option<String>>`, `Some(None)` still produces a present `null`, so the
emitted field is correctly optional **and nullable**.

The test also confirms that ordinary Serde deserialization maps that JSON null
back to outer `None`. Structural wire fidelity does not guarantee preservation
of every Rust value state. Partial-update semantics need deliberate codecs where
those distinctions matter.

Other omission predicates conservatively make a field optional without inferring
constraints from arbitrary Rust function bodies.

## 5. Explicit custom-codec descriptions

The ordinary field path rejects a field marked as having a custom codec unless a
description is supplied. The override path is:

```rust
cx.field_with::<String>("count", rules)
```

The provider describes this field's **present wire values** in the selected
direction. The tested `#[serde(with = "decimal")]` codec really encodes a `u64` as
a string. The provider can be a schema-only marker type; the original field type
does not need to implement `Schema` when an override is supplied.

The derive follow-up also established that a present-value override cannot infer
the original type's missing-field behavior from its provider. `field_with` now
requires a declared default to make an input property optional, even when the
provider itself is nullable and accepts missing fields.

For field-level `deserialize_with`, absence does not invoke the callback. Without
a default it is an error, even if the explicit wire provider is `Option<String>`.
The field builder preserves this behavior.

Overrides remain author-supplied declarations. A test explicitly demonstrates a
string accepted by the structural `UserId` schema but rejected by its Rust parser.
Additional predicates and generated validation are separate capabilities.

## 6. Registration invariants

For a named type the registry reserves its key/name before calling `describe()`.
A repeated visit to that building type returns a reference; completion fills in
its body. Tests cover a real Serde recursive `Node` value and a pair of mutually
referencing descriptions. Inline recursion is rejected because it has no named
reference through which to close the cycle.

Each root registration is atomic. A working copy is committed only after
construction and graph validation succeed. Failure after visiting children,
swallowing a context error, or unwinding from a description leaves the original
registry unchanged. This rollback concerns registry state, not external side
effects in handwritten functions.

Validation checks references, directions, duplicate fields, empty unions,
unfinished registrations, logical names, and the numeric profile. References are
checked without expanding them recursively, allowing recursive graphs to seal.

Definitions use an ordered map. Registering the same roots in different orders
produces equal graphs in the tests; field order remains description order. This
assumes deterministic `info()` and `describe()` implementations. There is no
attempt to prove purity of handwritten code.

The prototype clones the existing registry and revalidates it per root for
clarity. Journaling and incremental validation are possible future optimizations;
performance at scale has not been evaluated.

## 7. Connection to the private attribute adapter

The library imports neither Serde nor a parser. The test-only bridge:

1. Declares a DTO once and compiles it with real Serde derives.
2. Inspects the same declaration with the previous adapter experiment.
3. Turns names, docs, defaults, skip flags, and codec facts into field rules.
4. Uses explicitly typed calls for referenced schemas.
5. Compares actual JSON values with the resulting graph.

The typed dispatch table stands in for generated code. It never interprets
`field.rust_type` as a resolver. `User` and `Encoded` exercise this bridge; other
fixtures use handwritten, generated-code-shaped descriptions. A `Schema` derive
macro is the proposed next step.

The bridge's support checks cover its fixtures. Production lowering must validate
all representation-affecting attributes, retain diagnostic spans, and omit calls
and bounds for truly skipped fields. Recognition of omission function paths needs
a precise supported policy in that derive.

The graph describes chosen canonical JSON shapes and available structural facts,
not every incidental form a decoder might accept. Unknown-field policy is explicit;
the tagged-error fixture tests its emitted representation. The graph matcher lives
only in tests and is not a production validation engine.

## Evidence and reproduction

From the repository root:

```sh
cargo test --locked --manifest-path research/schema-registry-probe/Cargo.toml
cargo fmt --manifest-path research/schema-registry-probe/Cargo.toml -- --check
cargo clippy --locked --manifest-path research/schema-registry-probe/Cargo.toml --all-targets -- -D warnings
cargo tree --locked --manifest-path research/schema-registry-probe/Cargo.toml --edges normal
```

Verified on rustc 1.95.0-nightly (859951e3c, 2026-02-24):

- **11 registry tests:** identity, order, mutual recursion, collisions, rollback,
  ignored errors, unwinding, and malformed graph rejection.
- **9 wire-shape tests:** directional DTOs, nested Options, missing-field behavior,
  custom codecs, tagged errors, recursive values, and the numeric profile.
- Formatting and Clippy with warnings denied pass.
- The normal dependency tree contains only the probe crate itself.

Serde 1.0.229, serde_json 1.0.154, and the adapter's pinned dependencies are recorded
in the lockfile. The tests establish the listed cases, not exhaustive equivalence
with every Serde implementation.

## Recommended next step

Use this API as the target for a narrow **`#[derive(Schema)]`**: emit the typed
descriptions currently written by hand, using the evaluated private adapter and
an explicit support matrix.

That [derive step is now implemented](schema-derive.md), with its own behavior
tests, support matrix, and compiler-failure fixtures.

The next authoring decisions are logical-name overrides, custom-codec schema
attributes, and the public field-builder surface. The graph also gives a concrete
starting point for a small TypeScript exporter. Shared-versus-adapted Rust client
DTOs and asymmetric Serde behavior remain contract-level decisions.
