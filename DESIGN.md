# Spline design notes

## Status

This document summarizes the implemented design and its broader direction.
The root workspace implements directional schemas, derives, TypeScript export,
neutral Tower execution, and an HTTP/JSON binding with executable Rust and
TypeScript clients. See [the workspace guide](docs/workspace-contracts.md) and
[HTTP profile](docs/http-binding.md) for supported APIs and verification. The
[research notes](research/README.md) preserve the original investigations; the
[roadmap](ROADMAP.md) records upcoming milestones.

The [evolution policy](docs/contract-evolution.md) now defines compatible and strict
response decoding and verifies older-contract clients against newer servers.
Input validation remains authored by the request schema; output enum variants
remain closed even when additive object properties are tolerated.

The [update profile](docs/http-updates.md) adds PUT/PATCH/DELETE, a typed `Patch<T>`
field representation, and distinct 204/bodyless success handling in both clients.

[Application composition](docs/application-composition.md) now provides `spline::api!`,
grouped native/TypeScript clients, a shared request-scoped service factory, and
cross-resource schema/route validation. The [blog tutorial](examples/blog/README.md)
demonstrates the application-facing API through separate contract, server, and
Rust client crates plus a TypeScript package, sharing the root workspace and lockfiles.

[Execution and bindings](docs/execution-bindings.md) now separates neutral contracts
and application composition from HTTP descriptions and runtime. Typed Tower
operation services provide a shared execution boundary for in-process and HTTP
calls, with explicit HTTP APIs under `resource::http`/`api::http`.

## Purpose

Spline lets an application define its API contract in Rust and derive the pieces
needed to implement and consume it:

- A server interface with typed operations.
- Typed Rust and TypeScript clients.
- Operation and data-type metadata.
- Documentation and binding-specific specifications.

The contract is independently describable and can live in a shared contract crate.
Server implementations supply application behavior, while bindings determine how
calls cross a transport boundary.

## Agreed direction

| Area | Direction |
| --- | --- |
| Authoring | Rust traits and data types form the source contract. |
| Organization | Group operations by resource or domain, with custom operations supported. |
| Operation model | Named queries and mutations, initially unary request/response. |
| Clients | Rust and TypeScript are both first-class targets. |
| Transport | Keep operation identities and types independent of a particular transport. |
| First binding | HTTP + JSON, integrated with Axum. |
| Composition | Use Tower at runtime composition boundaries. |
| Serialization | Use Serde for encoding and decoding. |
| Schema ownership | Use Spline's directional schema graph and derives, with a private Serde-internals adapter. |
| Tooling | Generate clients and documentation from shared contract metadata; add specification exporters from that foundation. |

Optional inline HTTP annotations, discriminated TypeScript results, explicit export
programs, request-scoped implementations, and typed Tower adapters are implemented.
Bindings own their extension traits and transport concerns; neutral contracts and
shared service bundles remain usable without selecting HTTP.

## Resource-grouped contracts

Smaller contracts give operations context and allow concise method names:

```rust
// users.rs
#[spline::contract]
pub trait Users {
    /// Fetch a user by their ID.
    #[query]
    #[http(get = "/users/{id}")]
    async fn get(&self, id: UserId) -> Result<User, ApiError>;

    /// List users matching a name prefix.
    #[query]
    #[http(get = "/users")]
    async fn list(&self, prefix: Option<String>) -> Result<Page<User>, ApiError>;
}

// posts.rs
#[spline::contract]
pub trait Posts {
    /// Create a new post.
    #[mutation]
    #[http(post = "/posts", body = "input", status = 201)]
    async fn create(&self, input: NewPost) -> Result<Post, ApiError>;

    /// Publish an existing post.
    #[mutation]
    #[http(post = "/posts/{id}/publish")]
    async fn publish(&self, id: PostId) -> Result<Post, ApiError>;
}
```

Here, `#[query]`, `#[mutation]`, and `#[http(...)]` are supported annotations consumed
by the contract macro. Remaining scalar arguments become query parameters under
their argument names: `prefix` becomes `?prefix=...`, or is omitted when null.
Types such as `Page<User>` are application-defined examples.

The generated operation identities and client organization follow the
namespaces:

| Operation | HTTP binding | Illustrative TypeScript call |
| --- | --- | --- |
| `users.get` | `GET /users/{id}` | `api.users.get(id)` |
| `users.list` | `GET /users` with optional `prefix` query parameter | `api.users.list(prefix)` |
| `posts.create` | `POST /posts` | `api.posts.create(input)` |
| `posts.publish` | `POST /posts/{id}/publish` | `api.posts.publish(id)` |

Rust and TypeScript clients preserve the authored positional method arguments.
`spline::api!` combines explicitly listed resource modules and supplies
`client.users().get(id).await` in Rust and `client.users.get(id)` in TypeScript.
Group names are explicit client accessors; resource namespaces retain the stable
operation identities. Logical input objects remain named in metadata and are
assembled internally by the generated clients.

Contract support is exposed in the trait's existing Rust module by default, with
the namespace inferred from the final module-path component. File and inline
modules use the same authoring pattern. Optional `module` and `namespace` settings
provide a separate companion or a pinned logical identity when needed.

### Semantics

- A **query** is declared read-only.
- A **mutation** may change state.
- Custom operations such as `publish`, `search`, and `calculate` are first-class.

These classifications are independent of HTTP methods. A complex read-only search
can be bound to POST with a structured body. Caching, invalidation, and retry
behavior need their own policies; a query annotation alone does not establish them.

Operation names and namespaces become logical identifiers. An explicit
`namespace = "users"` preserves the namespace across Rust module moves, and
`#[query(name = "get")]` preserves an operation's local name across method renames.
HTTP routes and parameter names remain independently authored wire mappings.

### Explicit bindings and resource conventions

Explicit HTTP annotations are colocated with operations and optional in the
transport-neutral contract model. This keeps public HTTP behavior visible and
gives tooling enough information to validate mappings when HTTP is selected.

A possible later `#[resource(...)]` shorthand could expand conventional CRUD
operations into the same binding metadata. Its syntax and defaults are open,
including the distinction between partial updates and full replacement. Resource
organization does not require every contract to implement a fixed CRUD lifecycle.

## Contract metadata and generation

The [contract implementation](docs/workspace-contracts.md) generates an implementable
server trait, typed input wrappers, Tower adapters, and metadata with directional
input/output/error references. The TypeScript exporter emits public DTOs, an
operation map, and positional call signatures. The [HTTP binding](docs/http-binding.md)
adds Axum dispatch and executable Rust/TypeScript clients.

The conceptual outputs are:

```text
Rust trait + DTOs
├── Server trait, typed inputs, and Tower operation services
│   ├── In-process Rust calls
│   └── HTTP dispatch and Rust client methods
└── Neutral contract metadata + directional data-type descriptions
    ├── TypeScript DTOs, operation map, and call signatures
    └── Selected HTTP binding description
        ├── Executable TypeScript fetch client
        └── Future OpenAPI exporter
```

Each operation description includes its stable identity, query/mutation kind,
named inputs, success type, declared error type, and documentation. Binding
metadata describes how that operation is exposed through a particular protocol.

Multiple Rust method arguments can be represented by a generated input object.
This gives bindings and metadata a uniform description while retaining ergonomic
language-specific client methods.

The trait macro can inspect method syntax, but cannot independently discover the
structure of an arbitrary referenced Rust type. A schema trait and derives provide
that information. The macro can emit references to those implementations, and a
compiled export program can collect the resulting descriptions.

Generation is deterministic through explicit export programs. CI checks the schema,
CRUD/composition/evolution, and blog artifacts for drift, then type-checks and
executes the generated clients. [Contributing](CONTRIBUTING.md#generated-artifacts-and-diagnostics)
lists all three generator commands.

## Transport and runtime boundaries

An operation and its HTTP binding describe different concerns:

```text
Operation
  identity: posts.create
  kind: mutation
  input: { input: NewPost }
  output: Post
  error: ApiError

HTTP binding
  method: POST
  path: /posts
  body: input
  success status: 201
```

A future WebSocket binding could invoke `posts.create` with its input object. The
implemented in-process adapter invokes the same typed operation service directly.
Both reuse the operation contract without interpreting HTTP paths or status codes.

The initial scope is transport-neutral unary operations with a defined JSON wire
representation. Additional codecs and streaming capabilities need their own
design and capability checks.

| Layer | Responsibility |
| --- | --- |
| Contract model | Operations, identities, types, declared errors, and documentation. |
| Schema layer | Describe data types and their supported wire representations. |
| Generated code | Typed inputs, implementation interfaces, dispatch, and client methods. |
| Runtime composition | Invocation policies and integration with Tower services/layers. |
| Binding | Transport-specific routing, argument extraction, encoding, and response mapping. |
| Application | Business logic and application state. |

Axum supplies the HTTP integration and a Tower middleware boundary. Generated
operation services expose `Service<Input, Response = Result<Output, Error>,
Error = Infallible>` over shared implementations. Outer Tower layers can add
timeouts, concurrency limits, readiness, and operational errors.

`spline::runtime::invoke` waits for readiness and invokes the same service instance.
Its `CallError<E>` distinguishes typed `Application(E)` from `Service(BoxError)`;
HTTP maps the latter to a framework response. The generated trait requires `Send`
futures while implementations retain normal `async fn` methods. See
[execution and bindings](docs/execution-bindings.md) for the exact boundaries and
configured-service entry points.

## HTTP binding

The implemented HTTP profile defines:

- **Inputs:** identify path parameters, query parameters, and the request body.
- **Success:** 200/201 JSON responses and bodyless 204 responses for unit outputs.
- **Errors:** typed application-error envelopes with an executable status policy,
  separate from framework and unmarked HTTP failures.

Declarative error/status metadata and OpenAPI export are the next milestones.
Per-operation response headers such as `Location` and `ETag` remain future work.

For example, the supported `body = "input"` mapping sends a `NewPost` directly as the
JSON request body. A successful creation returns a `Post` with status 201. The
generated HTTP client knows how to map its named arguments to that request and
decode the response into its typed result.

The server implementation can continue returning `Result<Post, ApiError>`.
Status mappings such as a not-found error becoming 404 belong to the HTTP binding.
An `ErrorPolicy<E>` implementation or closure supplies a 4xx/5xx status today; its
possible mappings are not yet included in exported metadata.

Binding validation rejects missing or conflicting argument mappings, unsupported
path/query shapes, route collisions, and inconsistent semantics such as a mutation
bound to GET. Query strings use URL form encoding with scalar values; complex query
objects and repeated/array conventions are outside the supported profile. The
separate HTTP description validates these rules without coupling neutral composition
to HTTP.

## Errors and request context

### Client errors

An implementation declares application outcomes through `Result<T, E>`. A remote
client also encounters transport, protocol, and framework-level failures.

The generated Rust HTTP client returns:

```rust
Result<User, spline::http::CallError<ApiError>>
```

`spline::http::CallError` has application, framework, unmarked HTTP, transport,
protocol, and encode variants. It is distinct from the neutral runtime's
application/service error wrapper.

The generated TypeScript result is a discriminated union with an explicit binding
failure parameter:

```typescript
type UsersResult<Value, ApplicationError, Failure = never> =
  | { ok: true; value: Value }
  | { ok: false; error: { kind: "application"; value: ApplicationError } | Failure };

// HTTP-bound signature:
// get(id: UserId): Promise<UsersResult<User, ApiError, UsersHttpFailure>>;
```

HTTP operation failures resolve to typed results. Invalid client URL/header
configuration can throw at construction. The [HTTP guide](docs/http-binding.md#rust-client)
defines the failure taxonomy and [TypeScript guide](docs/typescript.md) describes
the public names and editor-facing signatures.

Clients therefore have corresponding operation methods with richer failure types
than the server implementation interface. Client-level result wrappers do not
require an identical envelope in every wire binding.

### Request context

Authentication principals, tracing, deadlines, and other server-side context need
to remain separate from serialized operation arguments.

HTTP uses a lightweight request-scoped implementation holding shared application
state and an authenticated principal, preserving the authored `&self` trait shape.
`api::contract().http_with(factory, policy)` selects this adapter through
`HttpFactoryExt`; the factory receives HTTP `RequestContext` before decoding.

The [blog tutorial](examples/blog/README.md#2-implement-the-server) demonstrates
this with one `Session` implementing both resources. Neutral bundles use
`api::shared(implementation)` and select `.in_process()` or `.http(policy)` through
the respective extension traits; they contain no HTTP context.

## Spline-owned schema layer

These components have been [promoted into workspace crates](docs/workspace-contracts.md#crate-boundaries).
The research notes below preserve the supporting investigations and earlier API stages.

The first [schema research note](research/schema.md) examined relevant Rust
implementations and established the directional model, support boundaries, and
Serde behavior probes that informed the implementation.

The follow-up [reuse evaluation](research/serde-internals-reuse.md) led to an exactly
pinned `serde_derive_internals` dependency behind a private macro-side adapter,
supported by behavior comparisons and compiler-failure fixtures.

The [schema/registry investigation](research/schema-registry.md) established a
two-required-method `Schema` API, directional type registration, field-presence
composition, and explicit custom-codec descriptions. `spline-schema` implements
that model with a std-only normal dependency graph.

The [derive investigation](research/schema-derive.md) established the supported
lowering and diagnostics now implemented in `spline-macros`: logical-name and
codec-schema overrides, directional shapes, and recursive/generic DTOs. A defaulted
`Schema::missing_field()` query handles recursive transparent wrappers while
preserving its two required methods.

The [TypeScript exporter investigation](research/typescript-exporter.md) established
the first Rust-derive-to-generated-types path. The current exporter emits short
public DTO names, shares equivalent directions, and retains directional suffixes
when shapes differ. Real Serde samples, compiler assertions, artifact freshness,
and language-service checks verify the public surface.

The [HTTP implementation](docs/http-binding.md) consumes these schemas for URL
planning, conservative native-client DTO compatibility checks, and generated
TypeScript request/response validation.

Serde remains responsible for serialization and deserialization. Spline's schema
layer supplies the structural descriptions needed by exporters and tooling.

An illustrative derive is:

```rust
#[derive(serde::Serialize, serde::Deserialize, spline::Schema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: UserId,
    pub display_name: String,
}
```

The responsibilities are:

1. **Schema derive:** describe supported structs, enums, and referenced types.
2. **Registry:** collect descriptions, including shared and recursive types.
3. **Serde-aware interpretation:** account for supported representation attributes
   and differences between serialization and deserialization.
4. **Exporters:** generate TypeScript from those descriptions; future specification
   exporters can reuse the same graph and selected binding metadata.

The important requirement is fidelity to the wire representation. Serde's
`Serialize` and `Deserialize` traits execute logic; they do not provide a complete
static schema interface. Custom implementations can change representations
arbitrarily, so explicit overrides or manual descriptions are required for custom
codecs. Those declarations are not proofs of arbitrary codec behavior.

The derive supports owned named structs, unit/newtype/transparent wrappers,
supported externally/internally/adjacently tagged enums, directional renaming,
defaults, and field omission. The built-in profile includes Option, Vec, Box,
string-keyed maps, and signed/unsigned 8/16/32-bit integers. Flattening, untagged
enums, tuple structs, and broader numeric representations need explicit schema
descriptions or overrides; unsupported derive cases receive diagnostics.

Schemas distinguish Deserialize/input from Serialize/output. Request types match
what the server accepts; response types describe what it emits. Presence,
nullability, and missing-field acceptance are separate facts. Serde conformance
fixtures, typed-client checks, and generated runtime validators exercise those
descriptions.

## Documentation and specifications

Macros capture doc comments and type metadata in the neutral descriptions and
schema graph. The TypeScript exporter carries documentation into generated DTOs
and operations; standardized specification exporters remain follow-on work.

- **OpenAPI:** derive from contract metadata, data schemas, and the HTTP binding.
  RPC-style implementation methods are compatible with resource-oriented HTTP
  documentation. Public API support makes this a valuable follow-on milestone.
- **OpenRPC:** appropriate if a future binding adopts JSON-RPC 2.0 specifically.
- **Neutral tooling:** consume Spline's operation catalog and type descriptions.

The internal metadata model can evolve while the required exports are explored.
Whether to publish and version a standalone Spline specification is an open
question.

## Implemented milestones

The original query/mutation milestone is complete and has expanded into:

- A [CRUD workbench](examples/contract/README.md) with both clients, malformed-wire
  checks, partial updates, and 204 responses.
- [Old-contract/new-server fixtures](docs/contract-evolution.md) verifying compatible
  projection and strict response decoding without widening authored schemas.
- [Composed resources](docs/application-composition.md) with grouped clients, shared
  schemas, and whole-application validation.
- A [multi-crate authenticated blog](examples/blog/README.md) using actual server
  and client entry points plus TypeScript editor checks.
- [Neutral Tower execution](docs/execution-bindings.md) verifying readiness,
  middleware failures, shared backpressure, cancellation, and HTTP-independent calls.
- [Local benchmarks](benchmarks/README.md) with documented metadata/export/client
  timing boundaries and fixture checks in CI.

[Contributing](CONTRIBUTING.md#verify-a-change) records the current verification
commands. [The changelog](CHANGELOG.md) records the implemented API changes.

## Follow-on design questions

The [roadmap](ROADMAP.md#next-priorities) orders the next milestones: declarative
HTTP error/status metadata, OpenAPI export, and preparation for a published release.

| Area | Remaining questions |
| --- | --- |
| Error/status declarations | How does a separate HTTP policy declaration reference reusable errors and provide both executable behavior and per-status serialized shapes? How are custom dynamic policies documented? |
| OpenAPI | How should directional schemas, success/error envelopes, and framework responses be represented accurately for the implemented HTTP profile? |
| Client controls | What per-call cancellation, header, timeout, and deadline controls should generated clients expose? |
| Broader representations | Which additional Serde shapes, numeric codecs, asymmetric client DTOs, and response headers have concrete use cases and conformance evidence? |
| Additional bindings | How should new transports/codecs or streaming extend the unary operation/runtime boundary and declare their capabilities? |
| Packaging and evolution | What public versioning policy and crate/generated-client packaging should the first release establish? |

## References

- [Resource-oriented API design](https://google.aip.dev/121): resource organization
  and custom operations in an RPC model.
- [HTTP and gRPC transcoding](https://google.aip.dev/127): precedent for mapping
  named operations to resource-oriented HTTP bindings.
- [Tower Service](https://docs.rs/tower/latest/tower/trait.Service.html): runtime
  request/response composition and readiness.
- [Axum handlers](https://docs.rs/axum/latest/axum/handler/index.html): handler and
  future requirements for the initial server integration.
- [Serde data model](https://serde.rs/data-model.html): representation flexibility
  and the boundary between Rust types and serialized data.
- [OpenRPC specification](https://spec.open-rpc.org/): the JSON-RPC 2.0-specific
  interface description format.
- [tarpc](https://github.com/google/tarpc): a reference for trait-driven RPC
  ergonomics.
