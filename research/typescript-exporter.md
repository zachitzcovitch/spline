# TypeScript export from the schema graph

Implemented: 2026-10-13.

**Workspace update:** code, tests, and artifacts now live in
[`crates/spline-typescript`](../crates/spline-typescript). The
[workspace guide](../docs/workspace-contracts.md) covers the current commands and
new operation-signature export. The prototype commands below are historical.

## Result

The [exporter implementation](../crates/spline-typescript) completes the first
path from Rust data types to a generated artifact checked by another language:

```text
Rust DTOs + Serde + Schema derives
              ↓
directional schema registry
              ↓
TypeScript exporter
              ↓
generated.ts + compiler assertions
```

The checked-in [generated types](../crates/spline-typescript/typescript/generated.ts)
are produced by the executable Rust example. The example also serializes real
Serde values into [wire-samples.ts](../crates/spline-typescript/typescript/wire-samples.ts),
where TypeScript checks them with `satisfies` against those types.

Verification passed: **14 Rust tests**, TypeScript **5.9.3** in strict mode with
positive assertions and **25 expected-error cases**, generation freshness,
formatting, and Clippy with warnings denied.

## Exporter API

The library consumes the owned runtime graph. It has no parser or Serde dependency
in its normal dependency tree and does no filesystem I/O:

```rust
let mut registry = Registry::new();
registry.register::<User>(Direction::Deserialize)?;
registry.register::<User>(Direction::Serialize)?;
let graph = registry.finish()?;

let artifact = Typescript::new().export(&graph, &[])?;
// artifact.source: the complete TypeScript module
// artifact.symbols: logical DefinitionId -> exported symbol
```

Every named definition in the graph is exported. Optional `Root` entries name
inline shapes, such as an operation argument tuple or a list of users:

```rust
let mut registry = Registry::new();
let users = registry.register::<Vec<User>>(Direction::Deserialize)?;
let graph = registry.finish()?;
let roots = [Root::new("UserListInput", Direction::Deserialize, users)];
let artifact = Typescript::new().export(&graph, &roots)?;
```

Roots carry a symbol, direction, shape, and optional documentation. Since callers
can construct them directly, the exporter checks their references, direction,
shape validity, and numeric profile rather than assuming they passed registration.

## Names and deterministic artifacts

Default symbols retain the logical namespace and direction:

| Definition | TypeScript symbol |
| --- | --- |
| `api.User`, Deserialize | `api_User_Input` |
| `api.User`, Serialize | `api_User_Output` |
| `api.Page<api.User>`, Serialize | `api_Page_Of_api_User_End_Output` |

`Input` and `Output` mean the contract/server's accepted and emitted JSON shapes.
Generic instantiations are concrete aliases in this slice. Directions remain
separate even if two definitions have the same body.

A definition can be named explicitly:

```rust
let exporter = Typescript::new().name(
    DefinitionId {
        name: User::info().name,
        direction: Direction::Serialize,
    },
    "UserOutput",
);
```

The override updates references as well as the declaration. Unknown overrides,
reserved/invalid symbols, and collisions among overrides, defaults, or root names
produce errors.

Readable default names are not an injective encoding: `api.User` and `api_User`,
for example, both produce `api_User_Output`. That collision is explicitly tested
and requires an override. The exporter never chooses traversal-order suffixes.

Definitions and roots are emitted in final-symbol order. Rust tests compare
byte-identical source and symbol maps after reversing root and type-registration
order, and verify that adding an unrelated definition does not rename existing
symbols. There are no generated timestamps or machine-specific paths.

## Type mapping

| Schema shape | TypeScript representation |
| --- | --- |
| Null | `null` |
| Boolean | `boolean` |
| String | `string` |
| Supported fixed-width integer | `number` |
| String literal | Escaped double-quoted literal |
| Array | `T[]`, with parentheses around union elements |
| Tuple | `[T1, T2, ...]` |
| String-keyed map | `{ [key: string]: T }` |
| Object | Quoted properties with field-level `?` markers |
| Union | `T1 \| T2 \| ...` |
| Reference | The assigned definition symbol |

All shapes currently present in the graph vocabulary have a rendering. This does
not expand the derive's supported Rust/Serde surface; the fixture's tuple root,
for example, is supplied directly as a graph shape.

Array and map renderings use language syntax rather than global `Array<T>` or
`Record<K, V>` helpers. The compiler fixture intentionally exports symbols named
`Array` and `Record` to ensure they cannot break those renderings.

### Optional and nullable stay distinct

The actual artifact includes:

```typescript
export type api_User_Input = {
  "id": api_UserId_Input;
  "displayName": string;
  "nickname"?: string | null;
  "tags"?: string[];
  "bio"?: string | null;
};

export type api_User_Output = {
  "id": api_UserId_Output;
  "displayName": string;
  "nickname": string | null;
  "tags": string[];
  "bio"?: string;
};
```

The compiler checks that output must contain `nickname` and `tags`, that present
output `bio` cannot be null, and that IDs use the declared string codec. It also
checks `(string | null)[]` separately from a nullable array, and narrows tagged
error unions by their discriminator.

The fixture enables `strict`, `exactOptionalPropertyTypes`, and
`noUncheckedIndexedAccess`. These settings make omitted properties distinct from
explicit `undefined` and avoid assuming arbitrary map keys exist. Consumers using
different compiler settings get TypeScript's corresponding weaker checks.

### Unknown fields and empty objects

An input object whose schema ignores unknown fields receives a string index
signature with `unknown` values. Its declared properties keep their types.
An object whose schema rejects unknown fields gets declared properties only.

TypeScript's `{}` type includes non-nullish primitives, so a closed empty JSON
object is emitted as `{ [key: string]: never }`. A permissive empty object uses
`{ [key: string]: unknown }`. Tests cover the distinction.

Declared-property objects still use TypeScript's structural type system: excess
properties are rejected on fresh literals, while non-fresh objects with additional
properties can be assigned. The assertions explicitly demonstrate that limitation.
Runtime decoding and exact-object checks remain binding/Serde responsibilities.

### Numeric and codec boundaries

The exporter accepts the runtime's evaluated integer profile and rejects wider
integer nodes supplied directly through roots. `number` cannot enforce integer
ranges or integrality; those constraints remain in the schema/runtime contract.

The example's `UserId(u64)` uses a real decimal-string Serde codec and an explicit
string schema. Serializing `u64::MAX` and checking that sample as TypeScript proves
the fixture's wire agreement without mapping a raw large number to `bigint`.
Custom format constraints are not inferred from a string declaration.

## References and recursion

References are emitted as names, preserving graph sharing and avoiding recursive
inline expansion. TypeScript checks the generated self-recursive object, mutually
recursive objects, and transparent recursive array aliases.

The runtime graph can also contain descriptions that TypeScript cannot express,
such as `type Loop = Loop | null`. Before export, a target-specific pass detects
cycles formed only by references and unions. It stops following an unguarded path
when it reaches an object, array, tuple, or map. Alias-only cycles return an error
instead of producing a known circular-alias failure.

This is a TypeScript-target restriction, not a change to the core schema model.
The prototype has not been benchmarked against very large or deeply nested graphs.

## Literals and documentation

JSON property names and string literals are always quoted. The renderer escapes
quotes, backslashes, control characters, and JavaScript line separators. A Rust
test round-trips every JSON control character and Unicode separators through the
rendered literal; TypeScript fixtures check unusual property and literal types.

Container and field docs are rendered as JSDoc. Comment terminators are escaped,
and line endings are normalized. A fixture includes apparent TypeScript source in
a Rust doc comment; the compiler verifies that it did not become an export.

## Reproduction and artifact workflow

From the repository root:

```sh
cargo run --locked --manifest-path research/typescript-exporter-probe/Cargo.toml --example export -- --write
cargo run --locked --manifest-path research/typescript-exporter-probe/Cargo.toml --example export -- --check
cargo test --locked --manifest-path research/typescript-exporter-probe/Cargo.toml
cargo fmt --manifest-path research/typescript-exporter-probe/Cargo.toml -- --check
cargo clippy --locked --manifest-path research/typescript-exporter-probe/Cargo.toml --all-targets -- -D warnings
npm --prefix research/typescript-exporter-probe/typescript ci --ignore-scripts --no-audit --no-fund
npm --prefix research/typescript-exporter-probe/typescript run check
```

`--write` regenerates both TypeScript artifacts from the Rust fixtures. `--check`
only compares them and fails on drift. The Rust suite also compares checked-in
artifacts byte-for-byte with fresh output. Compiler assertions use
`@ts-expect-error`, so an unexpectedly accepted invalid assignment fails the check.

Verification used rustc 1.95.0-nightly (859951e3c, 2026-02-24), Node v25.2.1, and
locally pinned TypeScript 5.9.3. Both Cargo and npm lockfiles are included. The
exporter's only normal dependency is the std-only schema runtime; the derive and
Serde are dev dependencies used by fixtures and the executable example.

## Next decision

We now have an executable Rust-to-TypeScript schema path with reproducible
artifacts. This is a good point to regroup on promoting the runtime, derive, and
exporter into a coherent crate layout and deciding the first contract-metadata
slice.

The next vertical feature could describe one query and one mutation, then use
these schema references to generate TypeScript operation signatures. Resource
composition, error wrappers, and explicit HTTP mappings can be evaluated against
that small contract rather than widening the schema subsystem speculatively.
