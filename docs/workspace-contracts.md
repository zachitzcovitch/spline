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
the CRUD conformance example explicitly names a generated companion module `users`.
The [blog tutorial](../examples/blog/README.md) uses the new same-module default.

The running source of truth is [examples/contract/src/lib.rs](../examples/contract/src/lib.rs).
It began with `users.get` (query) and `users.create` (mutation), and now also
implements replace, update, and delete through the [extended HTTP profile](http-updates.md).
Its tests decode generated input structs, call a real Rust implementation, and
compare its TypeScript artifact with a fresh export.

## Crate boundaries

```text
spline                         user-facing facade
├── spline-schema              Schema, registry, directional graph
├── spline-contract            operation metadata; depends on spline-schema
├── spline-runtime             typed Tower services and invocation
├── spline-macros              proc macros; private Serde-internals adapter
└── serde                      generated request decoding / output trait checks

spline-http                    optional facade feature; Axum + reqwest + JSON
└── spline-http-contract        separate HTTP descriptions and validation

spline-typescript
├── spline-schema
├── spline-contract
└── spline-http-contract        std-only binding metadata, not HTTP runtime
```

The schema, contract metadata, and exporter normal dependency trees have no
external runtime dependencies. The proc-macro crate pins `serde_derive_internals`
0.30.0 and Syn 3.0.3. The facade integrates Serde 1.0.229 for generated inputs.

One root Cargo lockfile now covers the workspace, and one npm workspace/lockfile
covers the TypeScript checks, including the [blog tutorial](../examples/blog/README.md).
The earlier standalone Serde behavior and internals probes have been promoted
to test-only modules and integration tests in `spline-schema`. They use the root
lockfile and run in CI; the schema runtime retains its std-only dependency graph.

The [research index](../research/README.md) maps the former package locations to
their workspace replacements. Historical design notes remain available; their
recorded prototype commands are superseded by the root commands below.

## Contract authoring and expansion

```rust
// users.rs, loaded by `pub mod users;`
#[spline::contract]
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
Generated per-operation Tower services adapt this static trait dispatch. They
are shared by the in-process and HTTP bindings; see [execution and bindings](execution-bindings.md).

### Generated resource support

By default the trait's existing Rust module is the resource entry point. A trait
in `users.rs`, loaded by `pub mod users;`, exposes:

```text
users
├── Users
├── contract() -> Definition
├── shared(implementation) -> Services<S>
├── get::{Input, Operation, Service<S>}
├── create::{Input, Operation, Service<S>}
├── in_process::Client<S>
├── http::{Client, router, router_with, describe}   # optional HTTP feature
├── describe() -> Result<ContractDescription, contract::Error>
└── register(&mut ApplicationBuilder, group) -> Result<(), contract::Error>
```

Input wrappers derive `Deserialize` and implement a deserialize-only `Schema`, reject
unknown named arguments, and expose public fields for typed dispatch. A
zero-argument operation gets an empty named input object. Multiple arguments are
fields of one object; optional argument types follow the established Serde
missing-field rules.

HTTP-bound contracts additionally generate input serialization and both schema
directions for the native HTTP client when the feature is enabled. Their `http`
module exposes `Client`, `router`, `router_with`, and a separate bound description.
Neutral `describe()` and `in_process` remain usable without HTTP.

The definition and bundle are neutral values. `HttpExt`, `HttpFactoryExt`, and
`InProcessExt` provide adapter selection on them, with generated bridges retaining
the resource's concrete implementation and error-policy bounds. See
[extension selection](execution-bindings.md#neutral-bundles-and-protocol-extensions).

Generated inputs must be `DeserializeOwned + Send`; success and application-error
types must be `Serialize + Send`. All referenced wire types need schema
descriptions. These are the neutral contract/server-facing requirements. The HTTP
profile also needs input serialization and output/error deserialization, with a
construction-time compatibility check for shared Rust DTOs.

Types are resolved through hidden aliases emitted in the trait's original module.
This preserves relative type paths and prevents a user DTO named `Input` from
being captured by the generated `Input`. A hidden runtime import similarly
preserves a renamed facade path across generated submodules. Tests exercise both
cases, including a raw-identifier operation name.

The same default works in an inline `pub mod users { ... }`. No attribute on the
module is needed. Use one default contract per module. If you want multiple
contracts in the same module, or existing items occupy generated support names,
set `module = "requests"` to create a separate companion module. Generated support
uses explicit imports so a conflicting source item is a compiler error rather
than silently replacing a generated entry point.

### Identity and options

- Without options, the namespace is the final component of the authored Rust
  module path (`users` in `crate::users` or `crate::domain::users`). At the crate
  root it is the crate name. Rust's declared module determines it, not a filename.
- `namespace = "v1.users"` optionally pins the namespace independently of Rust
  organization. Namespaces are dot-separated ASCII identifiers. Default namespaces
  can collide across nested modules; use overrides when composing such resources.
- `module = "requests"` optionally creates a companion module. If `namespace` is
  omitted in that form, the namespace defaults to the companion's name (`requests`).
- `crate = "renamed_facade"` identifies a renamed facade path.
- `#[query(name = "get")]` or `#[mutation(name = "create")]` pins the local wire
  operation name independently of the Rust method name.

For example, Rust `lookup` with `#[query(name = "get")]` keeps wire identity
`users.get` and input-schema identity `users.get.Args`, while its Rust input type
is `users::lookup::Input` for the `users` resource module. Metadata retains both names.

Previously, omitting `module` created a trait-derived module such as
`users_contract`. Put the trait in a resource module for the new default, or add
`module = "users_contract"` to preserve that earlier layout explicitly.

The first supported contract surface is a safe non-generic trait containing async
method declarations with an unannotated `&self`, named arguments, concrete owned
wire types, and a syntactic `Result<Output, Error>` return. Standard qualified
Result paths are also accepted; generated return types use core's Result.

Each method needs exactly one query/mutation annotation. The
[HTTP profile](http-binding.md) now supports explicit GET/POST/PUT/PATCH/DELETE annotations.
Unsupported attributes receive diagnostics. Generic operations,
associated items, borrowed wire parameters, Self-dependent types, custom Result
aliases, defaults/bodies, supertraits, and conditional method annotations are
outside the supported surface. Broader binding design questions are described in
[DESIGN.md](../DESIGN.md#http-binding).

## Metadata construction

`spline-contract` provides an independent `ContractBuilder` for consumers that
want to construct metadata directly. The macro's `describe()` uses this builder.

An operation records:

- Fully qualified ID, local name, and original Rust method name.
- Query/mutation kind and documentation.
- Input, success, and application-error shapes.
- Authored argument order. HTTP plans belong to a separate binding description.

The input is registered in the `Deserialize` direction. Success and error types
are registered in the `Serialize` direction. Shared definitions are collected in
the contract's single schema graph.

Adding an operation is transactional: all three shapes must register successfully
before any schema state or operation entry is committed. Tests cover an output
profile failure after successful input registration and subsequent recovery.
Empty contracts, invalid identifiers, and duplicate operations are rejected.

Operations and definitions are stored in ordered maps. Reversing declaration
order produces equal descriptors and byte-identical TypeScript. The result is a
sealed `ContractDescription` with read-only accessors. `ApplicationBuilder` now
registers multiple resources into one shared registry, preserving ownership and
transactional checks across contracts. See [application composition](application-composition.md)
for the generated router/grouped clients and metadata-only registration API.

## TypeScript operation export

```rust
let description = users::describe()?;
let artifact = spline_typescript::Typescript::new()
    .export_contract(&description, "Users")?;
```

The [generated artifact](../examples/contract/typescript/generated.ts) contains:

1. Clean public DTO names; matching directions share one declaration and differing
   shapes use `Input`/`Output` suffixes.
2. `UsersOperations`, keyed by stable IDs with literal query/mutation kinds and
   associated input/output/error types.
3. `UsersCalls<Failure = never>` and `UsersResult`, with methods keyed by local wire
   operation names and signatures referencing DTOs directly.

A method's callable type is equivalent to:

```typescript
get(id: UserId): Promise<UsersResult<User, ApiError, Failure>>;
```

The default exposes only declared application errors. A binding can supply an
additional tagged failure union, such as `{ kind: "transport"; message: string }`.
This is an extension slot for type signatures, not a finalized transport-error
taxonomy for all bindings. `export_http_client` now supplies a fetch implementation
and the first HTTP failure union; see [its wire format and behavior](http-binding.md).
The logical named input object stays in metadata, while public calls are positional.

Export checks collisions among public DTOs, generated utility names, the `Failure`
parameter, and the global `Promise` type. Schema symbol overrides update references
consistently. Generated dispatch wrappers and per-operation aliases remain outside
the public type exports; operation-map entries describe logical inputs directly.
See [TypeScript naming and editor support](typescript.md).

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

Use `--write` instead of `--check` to intentionally update the checked-in artifacts.
Rust tests also check artifacts for drift. [Contributing](../CONTRIBUTING.md#generated-artifacts-and-diagnostics)
lists all generators, including the blog application.

## Follow-up

The [HTTP binding](http-binding.md), including [updates and 204 responses](http-updates.md),
is implemented and verified with both clients against one server. Follow-on profile
decisions include client call controls and broader response metadata. [Declared
HTTP error/status metadata](http-errors.md) now supplies per-status application-error
shapes for the future OpenAPI exporter.
