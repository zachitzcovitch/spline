# Schema-layer research

Reviewed: 2026-10-12.

This note builds on the [design discussion](../DESIGN.md#spline-owned-schema-layer).
It records primary-source findings, behavior probes, and recommendations for the
next design discussion. The proposed Spline interfaces below are not implemented.

Follow-up: the [Serde internals reuse evaluation](serde-internals-reuse.md) now
contains a working private-adapter experiment and recommends reuse for the next
schema prototype.

The subsequent [trait/registry design and prototype](schema-registry.md) implements
that runtime boundary and tests directional shapes, field semantics, generic
identity, recursion, and transactional registration.

The [derive prototype](schema-derive.md) subsequently connects the compiler-side
adapter to generated runtime descriptions, with a documented supported surface.

The [TypeScript exporter](typescript-exporter.md) now consumes those descriptions
and verifies generated directional types and real Serde samples with TypeScript.

## Summary

The strongest candidate is an explicit **schema trait + derive + registry** that
produces a small, normalized description of supported wire shapes. Serde continues
to perform serialization and deserialization.

The main lessons from the projects reviewed are:

1. Keep serialization, schema metadata, and service generation independently usable.
2. Interpret attributes once, validate them, and lower them into a representation
   that exporters can share.
3. Model serialization and deserialization separately from the beginning.
4. Track field presence independently from whether its value can be null.
5. Use references and explicit type identity to handle recursion and generics.
6. Define and test a supported Serde/wire-format subset, with explicit custom-codec
   descriptions where inference is insufficient.

The reviewed Tokio and BurntSushi projects supply useful architectural precedents.
Schemars is the closest direct implementation of the schema-generation problem.
This is a focused source review, not a claim about every crate those developers
have written.

## What existing Rust projects teach us

### Tokio ecosystem: prost and prost-build

[Prost][prost] generates ordinary Rust message types with derives, retains source
comments, and treats package names as module organization. Its core deliberately
does not provide full runtime reflection. `prost-build` can emit a
`FileDescriptorSet` for tooling that needs the original schema information.

The [`ServiceGenerator` interface][prost-build] takes service metadata and emits
application-specific interfaces or implementations. Its documentation explicitly
describes generating a service trait and dispatch glue for an RPC framework.

**Lesson for Spline:** contract metadata should be usable by code generators and
documentation tooling without putting those tools on every request's execution
path. A typed execution interface and a descriptor model serve different uses.

Prost is schema-first and Protobuf-specific, so it is not a mechanism for reflecting
arbitrary Serde types. Its FAQ also explains why Rust types alone do not determine
Protobuf field tags or encodings. A future Protobuf binding would need additional
wire-contract information; transport independence does not supply it automatically.

### BurntSushi: csv

The [CSV serializer][csv] has separate `SeRecord` and `SeHeader` serializers.
`SeHeader` observes Serde struct-field events to produce column names, using an
explicit state machine. The implementation rejects unsupported forms such as maps
and certain nested/header combinations, with tests for those boundaries.

**Lesson for Spline:** Serde's serialization interface can expose useful information
about a value, and format-specific support can be narrow and well defined. The CSV
header mechanism does not establish a complete schema for all possible values of
a type, all enum variants, or both serialization directions.

### BurntSushi's regex ecosystem: regex-syntax

[`regex-syntax`][regex-syntax] explicitly distinguishes its source-oriented `Ast`
from a simpler, normalized `Hir`. Parsing and translation are separate steps;
downstream consumers can work with the simpler representation. It also preserves
useful error reporting and has deliberate handling of deep structures.

**Lesson for Spline:** use a private representation for Rust syntax and Serde
attributes, then produce a normalized schema graph. TypeScript and specification
exporters should not each reinterpret Serde attributes. This is an architectural
analogy; `regex-syntax` is not itself a Rust type-schema system.

### Serde's derive machinery

Serde's [`Container::from_ast`][serde-ast] converts a Syn AST into its own container,
variant, and field model. It applies naming rules and runs validation before code
generation. Its [`Ctxt`][serde-ctxt] collects and combines span-aware errors.

**Lesson for Spline:** parse, validate, and generate in distinct stages. Attach
diagnostics to the relevant field or attribute, and report multiple useful errors
together where possible.

An especially relevant discovery: Schemars [uses][schemars-ast]
[`serde_derive_internals`][serde-internals] to interpret Serde syntax. The crate
lists David Tolnay (`dtolnay`) as its owner and is explicitly described as unstable.

We can own Spline's schema graph and exporters while evaluating this helper behind
a private, version-controlled adapter. The tradeoff is closer reuse of Serde's
attribute handling versus dependency on an unstable internal API. A small owned
parser for a documented subset is the other option. The subsequent
[evaluation](serde-internals-reuse.md) recommends private reuse. Neither approach
proves the behavior of arbitrary handwritten Serde implementations.

### Schemars: the closest direct precedent

The [`JsonSchema` trait][schemars-trait] separates a schema's readable name from its
identity. Generic parameters that affect the schema participate in that identity.

The [generator][schemars-generator] provides several particularly useful details:

- A schema's internal key includes its identity and serialize/deserialize contract.
- Nested types are registered and referenced rather than always expanded inline.
- A placeholder definition is inserted before descending into a type, breaking
  recursive generation cycles.
- Definitions and exported names are tracked separately.

[Serialize and deserialize settings][schemars-directions] affect field names,
presence, and skipped fields. This is concrete precedent for Spline's directional
model, rather than treating direction as a future extension.

Schemars emits JSON Schema directly. For Spline, a smaller owned graph could feed
both TypeScript and JSON Schema/OpenAPI exporters. That graph's exact scope needs
to be chosen against the examples below.

### serde-reflection: why probing Serde is not sufficient as the foundation

[`serde-reflection`][serde-reflection] extracts a registry by tracing serialization
of samples and/or exercising deserialization visitors. It is a valuable example of
using Serde as a source of structural information.

Its documented limits are directly relevant to Spline: tagged/flattened forms,
multiple instantiations of a generic type in a tracing session, colliding container
names, and some recursive types. Serialization samples also cannot reveal unseen
enum variants or the element type of every empty collection.

**Recommendation:** use behavior probes to check a declared schema. An explicit
schema trait is a better foundation for the proposed JSON-oriented API contracts.

### ts-rs: useful exporter ergonomics

[`ts-rs`][ts-rs] uses a derive-backed `TS` trait, handles referenced types/imports,
and provides overrides for unsupported or third-party types. Its documented Serde
support and treatment of skipped fields show the compromises involved in emitting
one type for both directions.

It also exposes a choice for how large integers appear in TypeScript. That is an
important reminder that a TypeScript declaration and a working JSON codec are
separate pieces. Spline needs to check their agreement.

## Proposed Spline model

### Pipeline

```text
Rust DTO + Serde attributes
          |
          v
private parsed model + attribute validation
          |
          v
generated Schema implementations, resolved by Rust's type system
          |
          v
registry + selected direction + wire profile
          |
          v
normalized named wire-shape graph
          |
          +--> TypeScript
          +--> JSON Schema / OpenAPI schemas
          +--> documentation
```

The macro emits references to other types' schema implementations. It does not
resolve arbitrary Rust type paths by scanning source files or interpreting type
names as strings. A compiled export program collects the graph.

The first profile should describe JSON wire shapes. HTTP path and query encoding
remain binding-level projections with their own supported shapes. Additional
transports can reuse the JSON profile; additional serialization formats require
an explicit representation design.

### Small graph, with field metadata

A candidate vocabulary is:

```text
Shape
  null | boolean | string
  integer { signedness, width } | float { width }
  literal
  array(element) | tuple(elements)
  object(fields, unknown-field policy)
  string-keyed map(value)
  union(variants)
  reference(definition)

Field
  wire name
  value shape
  presence: required | optional
  documentation
```

Definitions also carry identity, an export name, and documentation. Exact Rust
types for these nodes are intentionally still a proposal. Export/profile validation
must reject nodes whose representation has not been specified for that target.

In particular, nullable values are represented by a union containing `null`.
Optional object membership belongs to the field. These cannot share one flag.
Numeric widths should survive normalization even though TypeScript's `number`
cannot express their ranges.

Graph construction should reserve a definition before describing its children,
then fill the definition when complete. Repeated references to an in-progress
definition reuse its ID. An unsuccessful build must not export unfinished entries.

Start by describing concrete generic instantiations such as `Page<User>` and
`Page<Post>` independently. Preserving generic TypeScript syntax can be a later
exporter choice. Regular recursive types should resolve to graph references.

Keep internal identity separate from public names. An in-process `TypeId` could
help for owned, `'static` DTOs, but it should not become a persisted wire identifier.
Rust explicitly documents [`type_name`][type-name] as diagnostic, neither unique
nor stable. Export names need a deterministic naming policy, collision handling,
and an override mechanism. Traversal order should not silently rename public types.

### Direction means Serde direction

Use explicit `Serialize` and `Deserialize` terminology in the schema machinery:

| Boundary | Relevant schema |
| --- | --- |
| Server accepts operation arguments | Deserialize |
| Server emits a success value or declared error | Serialize |
| Rust client sends arguments | Its serialization must match what the server accepts. |
| Rust client receives a response | Its deserialization must preserve the emitted fields as intended. |

The registry key must distinguish direction. A single registry can hold both
views; an exporter may reuse one definition after establishing that the views are
equivalent.

## Three concrete examples

### 1. A struct: missing, nullable, and omitted are different

The [Rust probe](serde-wire-probe/src/lib.rs) uses this shape:

```rust
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct User {
    display_name: String,
    nickname: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bio: Option<String>,
}
```

The corresponding proposed TypeScript views are:

```typescript
type UserInput = {
  displayName: string;
  nickname?: string | null;
  tags?: string[];
  bio?: string | null;
};

type UserOutput = {
  displayName: string;
  nickname: string | null;
  tags: string[];
  bio?: string;
};
```

These are manually reasoned examples, not output from a Spline generator.
The probes confirm that a missing `nickname` becomes `None`, but serialization
emits `"nickname": null`; missing `tags` defaults to `[]`, but explicit `null` is
rejected; `bio: None` is omitted during serialization.

An additional probe shows why custom deserialization needs care: adding a
`deserialize_with` callback to an `Option<String>` field can make the missing field
an error unless a default is supplied. The Rust type alone cannot determine field
presence.

### 2. A tagged enum: lower representation before exporting

```rust
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "details", rename_all = "snake_case")]
enum ApiError {
    NotFound,
    Validation { field: String },
}
```

Its canonical output is described by:

```typescript
type ApiError =
  | { kind: "not_found" }
  | { kind: "validation"; details: { field: string } };
```

The unit variant has no `details` property. Normalize the enum into tagged object
alternatives once; exporters should consume those alternatives rather than
implementing Serde enum rules independently. Preserve useful variant documentation
through that normalization. See [Serde's enum representations][serde-enums].

### 3. A custom-serialized newtype: describe the codec's output

The probe defines `UserId(u64)` with handwritten Serde implementations that encode
and decode a decimal string. For `u64::MAX`, its JSON is:

```json
"18446744073709551615"
```

The schema must describe a string, even though the Rust field is a `u64`. A manual
schema implementation or explicit override can supply that description. The final
override syntax is still open.

Schema metadata does not change Serde's encoding. Changing only a TypeScript type
or schema annotation would leave the actual numeric JSON representation unchanged.
Decimal syntax and range validation are additional constraints; a TypeScript
`string` declaration alone does not enforce them.

## Findings that affect the wider contract design

### Shared Rust DTOs need a compatibility policy

This Serde attribute is valid:

```rust
#[serde(rename(serialize = "out", deserialize = "in"))]
name: String
```

For the required field in the probe, serializing the struct produces `"out"`, but
deserializing that same JSON fails because `"in"` is missing. Sharing the type
between Rust client and server does not fix this.

Directional schemas can describe the two shapes, but a Rust client also needs
codecs that use the right shape. One-sided skipping can likewise lose fields or
prevent a client from sending required arguments.

**Proposed initial policy:** support shared DTOs whose Serde producer/consumer
representations are compatible for their uses. Diagnose known incompatible
attribute combinations. Full asymmetric support would need explicit adapters or
distinct generated wire DTOs. This is an open design decision, not an agreed
restriction. Even compatible DTOs still benefit from separate schema directions,
as the optional/defaulted fields above demonstrate.

### The JSON/TypeScript numeric policy must be explicit

The Rust probe confirms that raw `u64::MAX` becomes an unquoted JSON number. The
[JavaScript probe](serde-wire-probe/json-numbers.mjs) confirms that ordinary
`JSON.parse` loses precision for that value, and conversion to `bigint` afterward
cannot recover it. Ordinary `JSON.stringify` also rejects a `bigint` value.

**Recommendation:** initially require an explicit wire representation for large
integers exposed to TypeScript, such as a decimal-string codec. A lossless numeric
parser would be a different binding design. The exporter should not silently
choose `number` or `bigint` and claim that the codec problem is solved.

The probe also confirms that `serde_json` emits `null` for non-finite `f64` values,
while deserializing `null` as `f64` fails. Float support needs a finite-value policy
enforced by a validated type or runtime boundary, or an explicitly different
representation. Platform-sized integers need a defined range policy as well.

### Define canonical shapes and the limits of schema guarantees

The probe demonstrates that `serde_json` can deserialize a named struct from a JSON
array as well as its usual object form. Default unknown-field handling is also
permissive unless `deny_unknown_fields` is used.

Spline should distinguish its documented canonical wire shapes from every
incidental form a decoder may accept. The proposed first profile uses object form
for named structs and records unknown-field policy. Whether bindings additionally
enforce canonical forms is a separate decision.

Schemas describe structural shapes and supported constraints. Arbitrary validation
inside `try_from` or handwritten `Deserialize` implementations remains executable
logic. TypeScript declarations are not runtime validation, and schema generation
cannot prove arbitrary application predicates.

## Candidate first support boundary

This is a proposal for a small implementation slice, not a compatibility promise.

| Feature | Proposed treatment |
| --- | --- |
| Strings, booleans, fixed-width integers up to 32 bits, unit | Direct shapes; unit is JSON null, with bodyless HTTP responses handled by the binding. |
| `Option`, `Vec`, string-keyed maps, named structs | Support with separate presence/nullability and explicit unknown-field policy. |
| Simple newtypes, `transparent`, references through `Box` | Describe the underlying wire value while retaining a useful named definition. |
| Concrete generic DTOs and regular recursive types | Register identities before walking dependencies; test collisions and cycles. |
| `rename`, `rename_all`, doc comments | Normalize names per direction and preserve documentation. |
| `default`, `skip`, `skip_serializing`, `skip_deserializing` | Model each direction; also check compatibility with the chosen Rust client strategy. Default presence does not require executing a default function during export. |
| `skip_serializing_if = "Option::is_none"` | Model omitted output and non-null present values for the supported standard Option case. Other predicates need an explicit precision policy. |
| Externally/adjacently tagged enums; internal tags on supported object/unit variants | Lower supported cases into tagged alternatives; validate Serde's representation restrictions. |
| `with`, `serialize_with`, `deserialize_with`, handwritten implementations | Require an explicit compatible description where automatic derivation cannot establish the shape. Custom missing-field behavior must also be represented. |
| `flatten`, untagged/mixed enums, aliases, remote derives, conversion attributes | Evaluate separately; unsupported representation-affecting cases should produce diagnostics rather than guessed shapes. |
| Large integers, platform-sized integers, floats, non-string map keys | Require a documented profile/codec policy before claiming support. |

An owned type can provide a manual schema implementation. Foreign types can use
local wrappers, field adapters, or integrations provided by Spline; ordinary Rust
orphan rules still apply.

The derive can detect unsupported visible attributes, but it cannot inspect every
handwritten Serde implementation elsewhere in a crate. Manual descriptions and
custom-codec overrides need author-supplied conformance evidence. There should be
no promise that every `Serialize` type is automatically representable.

## Reproducible evidence

[The isolated probe crate](serde-wire-probe/Cargo.toml) pins Serde 1.0.228 and
serde_json 1.0.145, with a lockfile. It exercises Serde directly and does not depend
on a Spline implementation. These pinned releases are distinct from the upstream
source snapshots reviewed above.

From the repository root:

```sh
cargo test --locked --manifest-path research/serde-wire-probe/Cargo.toml
node research/serde-wire-probe/json-numbers.mjs
```

The research run used rustc 1.95.0-nightly (859951e3c, 2026-02-24) and Node v25.2.1:
all 10 Rust behavior tests passed, along with the JavaScript precision/bigint
checks. They cover the three examples above plus asymmetric names, custom Option
deserialization, unknown fields, struct sequence input, and numeric edge cases.

The next schema prototype should compare its graph and exported types against
these behaviors, adding compile-fail cases and graph-specific checks for recursion,
generic identity, naming collisions, and deterministic output.

## Recommended next discussion

Agree on these three choices before implementing the schema engine:

1. **Representation:** a small directional wire-shape graph, with explicit presence
   and references, initially normalized for a JSON profile.
2. **Attribute parsing:** the [completed evaluation](serde-internals-reuse.md)
   recommends `serde_derive_internals` behind a private, version-pinned adapter.
   Carry that boundary into the next schema prototype.
3. **Rust client compatibility:** decide whether the first version requires
   compatible shared DTOs or also generates/adapts asymmetric wire DTOs.

The [trait/registry experiment](schema-registry.md) now exercises `User`, `ApiError`,
`UserId`, concrete `Page<T>` instantiations, and recursive types. It recommends a
narrow derive implementation as the next step toward generated descriptions.

## Source snapshots

Repository links below are pinned to the revisions reviewed; documentation-site
links describe their published material at the review date. These are design
references, not proposed dependency selections.

[prost]: https://github.com/tokio-rs/prost/blob/aed74ad0e844959ee6948f29c4e9cff0f278e89e/README.md
[prost-build]: https://github.com/tokio-rs/prost/blob/aed74ad0e844959ee6948f29c4e9cff0f278e89e/prost-build/src/lib.rs
[csv]: https://github.com/BurntSushi/rust-csv/blob/05612e87e6e92910d40d7214b70952b8434fef9b/src/serializer.rs
[regex-syntax]: https://github.com/rust-lang/regex/blob/72d650cb0a880a01ab6dc2137c0888e8f89740f7/regex-syntax/src/lib.rs
[serde-ast]: https://github.com/serde-rs/serde/blob/6693a89cca77e0151437da1c7f890090b9ebf04c/serde_derive/src/internals/ast.rs
[serde-ctxt]: https://github.com/serde-rs/serde/blob/6693a89cca77e0151437da1c7f890090b9ebf04c/serde_derive/src/internals/ctxt.rs
[serde-internals]: https://docs.rs/serde_derive_internals/0.30.0/serde_derive_internals/
[schemars-ast]: https://github.com/GREsau/schemars/blob/4791bb7352555d339e45658ca09c092ed9fa1f5d/schemars_derive/src/ast/mod.rs
[schemars-trait]: https://github.com/GREsau/schemars/blob/4791bb7352555d339e45658ca09c092ed9fa1f5d/schemars/src/lib.rs
[schemars-generator]: https://github.com/GREsau/schemars/blob/4791bb7352555d339e45658ca09c092ed9fa1f5d/schemars/src/generate.rs
[schemars-directions]: https://graham.cool/schemars/generating/#serialize-vs-deserialize-contract
[serde-reflection]: https://github.com/zefchain/serde-reflection/blob/efea8e451e3204dfcf417ccf61961245f025bab4/serde-reflection/README.md
[ts-rs]: https://github.com/Aleph-Alpha/ts-rs/blob/06baebd270abc57ff2caba1569bdbae3999e48a3/README.md
[type-name]: https://doc.rust-lang.org/std/any/fn.type_name.html
[serde-enums]: https://serde.rs/enum-representations.html
