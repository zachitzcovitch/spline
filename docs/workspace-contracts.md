# Workspace and first contract-metadata slice

Implemented: 2026-10-13.

## Milestone

The schema runtime, derive, and exporter have been promoted from isolated research
packages into the root Cargo workspace. Their conformance tests and generated
TypeScript fixtures moved with them. The new contract slice connects an annotated
Rust trait to an implementation interface, typed input structures, operation
metadata, and TypeScript call signatures.

The subsequent [HTTP binding milestone](http-binding.md) adds executable clients
and Axum dispatch. Public calls now preserve authored positional parameters, and
the example explicitly names its generated companion module `users`.

The running source of truth is [examples/contract/src/lib.rs](../examples/contract/src/lib.rs).
It has exactly two operations: `users.get` (query) and `users.create` (mutation).
Its tests decode generated input structs, call a real Rust implementation, and
compare its TypeScript artifact with a fresh export.

## Crate boundaries

```text
spline                         user-facing facade
├── spline-schema              Schema, registry, directional graph
├── spline-contract            operation metadata; depends on spline-schema
├── spline-macros              proc macros; private Serde-internals adapter
└── serde                      generated request decoding / output trait checks

spline-http                    optional facade feature; Axum + reqwest + JSON

spline-typescript
├── spline-schema
└── spline-contract
```

The schema, contract metadata, and exporter normal dependency trees have no
external runtime dependencies. The proc-macro crate pins `serde_derive_internals`
0.30.0 and Syn 3.0.3. The facade integrates Serde 1.0.229 for generated inputs.

One root Cargo lockfile now covers the workspace, and one npm workspace/lockfile
covers both TypeScript test suites. The earlier standalone Serde behavior and
internals probes remain excluded research packages. The schema tests retain a
dev-only dependency on the internals probe for its existing comparison fixtures.

The [research index](../research/README.md) maps the former package locations to
their workspace replacements. Historical design notes remain available; their
recorded prototype commands are superseded by the root commands below.

## Contract authoring and expansion

```rust
#[spline::contract(namespace = "users", module = "users")]
pub trait Users {
    #[query]
    async fn get(&self, id: UserId) -> Result<User, ApiError>;

    #[mutation]
    async fn create(&self, input: NewUser) -> Result<User, ApiError>;
}
```

The trait remains named `Users`, with `Send + Sync` supertraits. Each method is
lowered to an RPITIT signature equivalent to:

```rust
fn get(&self, id: UserId)
    -> impl Future<Output = Result<User, ApiError>> + Send;
```

Implementations can continue using `async fn`. A compiler-failure fixture proves
that holding an `Rc` across an await cannot satisfy the generated future bound.
This is static trait dispatch; trait objects and server dispatch adapters are
separate work.

### Generated companion module

The default module is the snake-case trait name followed by `_contract`. The
example explicitly uses `module = "users"`, making the public entry point visible
in the authored declaration:

```text
users
├── get::Input { id: UserId }
├── create::Input { input: NewUser }
└── describe() -> Result<ContractDescription, contract::Error>
```

Input wrappers derive `Deserialize` and a deserialize-only `Schema`, reject
unknown named arguments, and expose public fields for a future dispatcher. A
zero-argument operation gets an empty named input object. Multiple arguments are
fields of one object; optional argument types follow the established Serde
missing-field rules.

HTTP-bound contracts additionally generate input serialization and both schema
directions for the native client. Their companion module exposes `Client`,
`router`, and `router_with`; normal server/client setup collects metadata internally.

Generated inputs must be `DeserializeOwned + Send`; success and application-error
types must be `Serialize + Send`. All referenced wire types need schema
descriptions. These are the metadata-only/server-facing requirements. The HTTP
profile also needs input serialization and output/error deserialization, with a
construction-time compatibility check for shared Rust DTOs.

Types are resolved through hidden aliases emitted in the trait's original module.
This preserves relative type paths and prevents a user DTO named `Input` from
being captured by the generated `Input`. A hidden runtime import similarly
preserves a renamed facade path across generated submodules. Tests exercise both
cases, including a raw-identifier operation name.

### Identity and options

- `namespace = "users"` is required. Namespaces are dot-separated ASCII identifiers.
- `module = "requests"` overrides the generated Rust module name.
- `crate = "renamed_facade"` identifies a renamed facade path.
- `#[query(name = "get")]` or `#[mutation(name = "create")]` pins the local wire
  operation name independently of the Rust method name.

For example, Rust `lookup` with `#[query(name = "get")]` keeps wire identity
`users.get` and input-schema identity `users.get.Args`, while its Rust input type
is `users::lookup::Input` when `module = "users"`. Metadata retains both names.

The first supported contract surface is a safe non-generic trait containing async
method declarations with an unannotated `&self`, named arguments, concrete owned
wire types, and a syntactic `Result<Output, Error>` return. Standard qualified
Result paths are also accepted; generated return types use core's Result.

Each method needs exactly one query/mutation annotation. The
[HTTP profile](http-binding.md) now supports explicit GET/POST annotations.
Unsupported attributes receive diagnostics. Generic operations,
associated items, borrowed wire parameters, Self-dependent types, custom Result
aliases, defaults/bodies, supertraits, and conditional method annotations are
outside this initial surface. The broader HTTP authoring proposal remains in
[DESIGN.md](../DESIGN.md#http-binding).

## Metadata construction

`spline-contract` provides an independent `ContractBuilder` for consumers that
want to construct metadata directly. The macro's `describe()` uses this builder.

An operation records:

- Fully qualified ID, local name, and original Rust method name.
- Query/mutation kind and documentation.
- Input, success, and application-error shapes.
- Authored argument order, plus a validated HTTP plan when a binding is declared.

The input is registered in the `Deserialize` direction. Success and error types
are registered in the `Serialize` direction. Shared definitions are collected in
the contract's single schema graph.

Adding an operation is transactional: all three shapes must register successfully
before any schema state or operation entry is committed. Tests cover an output
profile failure after successful input registration and subsequent recovery.
Empty contracts, invalid identifiers, and duplicate operations are rejected.

Operations and definitions are stored in ordered maps. Reversing declaration
order produces equal descriptors and byte-identical TypeScript. The result is a
sealed `ContractDescription` with read-only accessors. API-level composition across
multiple contracts is a follow-on feature.

## TypeScript operation export

```rust
let description = users::describe()?;
let artifact = spline_typescript::Typescript::new()
    .export_contract(&description, "Users")?;
```

The [generated artifact](../examples/contract/typescript/generated.ts) contains:

1. Directional DTO definitions and named input/output/error aliases per operation.
2. `UsersOperations`, keyed by stable IDs with literal query/mutation kinds and
   associated input/output/error types.
3. `UsersCalls<Failure = never>`, with methods keyed by local wire operation names.

A method's callable type is equivalent to:

```typescript
get: (id: example_UserId_Input) => Promise<
  | { ok: true; value: users_get_Output }
  | {
      ok: false;
      error: { kind: "application"; value: users_get_Error } | Failure;
    }
>;
```

The default exposes only declared application errors. A binding can supply an
additional tagged failure union, such as `{ kind: "transport"; message: string }`.
This is an extension slot for type signatures, not a finalized transport-error
taxonomy for all bindings. `export_http_client` now supplies a fetch implementation
and the first HTTP failure union; see [its wire format and behavior](http-binding.md).
The logical named input object stays in metadata, while public calls are positional.

Export checks collisions with operation type aliases, generated surface names,
the `Failure` parameter, and the global `Promise` type. Existing schema symbol
overrides continue to update referenced DTO names consistently.

## Verified behavior

The figures below record the original consolidation milestone. The
[HTTP guide](http-binding.md#evidence) records the expanded current suite.

The workspace test run passed **69 behavior/integration tests** plus the trybuild
harness containing **19 compiler-failure fixtures** (11 schema and 8 contract
fixtures). The new tests cover server implementation, decoded typed inputs,
schema directions, wire-name stability, relative paths, atomic metadata
construction, deterministic export, and call-surface naming conflicts.

Both TypeScript suites pass with TypeScript 5.9.3, strict mode,
`exactOptionalPropertyTypes`, and `noUncheckedIndexedAccess`. The original suite
has 25 expected-error assertions; the contract suite adds 9. They check operation
inputs, kinds, outputs, application-error narrowing, and explicit extra failures.

Generated-artifact freshness, root formatting, and workspace Clippy with warnings
denied pass. Verification used rustc 1.95.0-nightly (859951e3c, 2026-02-24) and
Node v25.2.1. This is the tested toolchain, not an MSRV commitment.

## Commands

```sh
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo run --locked -p spline-typescript --example export -- --check
cargo run --locked -p spline-example-contract -- --check
npm ci --ignore-scripts --no-audit --no-fund
npm run check
```

Use `--write` instead of `--check` for either generator to intentionally update its
checked-in artifact. Rust tests also check artifacts for drift.

## Follow-up

The first [HTTP binding](http-binding.md) is now implemented and verified with
both clients against one server. The next profile decisions concern additional
verbs and response forms, client call controls, and declarative error-status
metadata for specification export.
