# Spline design notes

## Status

This document captures the initial design discussion and the broader direction.
The root workspace now implements schemas, their derive and TypeScript exporter,
and a first HTTP/JSON binding with executable Rust and TypeScript clients.
See [the workspace guide](docs/workspace-contracts.md) and [HTTP profile](docs/http-binding.md)
for the supported API and verification. Broader syntax examples below can include
capabilities beyond that first profile.

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
| Schema ownership | Investigate and design a Spline-owned schema layer rather than adopting Specta. |
| Tooling | Generate clients and documentation from shared contract metadata; add specification exporters from that foundation. |

Working preferences to validate include optional inline HTTP annotations,
discriminated results in TypeScript, and an explicit export program for generated
artifacts. Request-context ergonomics, exact Tower service shapes, and the schema
model need deeper research.

## Resource-grouped contracts

Smaller contracts give operations context and allow concise method names:

```rust
#[spline::contract(namespace = "users")]
pub trait Users {
    /// Fetch a user by their ID.
    #[query]
    #[http(get = "/users/{id}")]
    async fn get(&self, id: UserId) -> Result<User, ApiError>;

    /// List users matching a filter.
    #[query]
    #[http(get = "/users", query = "filter")]
    async fn list(&self, filter: UserFilter) -> Result<Page<User>, ApiError>;
}

#[spline::contract(namespace = "posts")]
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

Here, `#[query]`, `#[mutation]`, and `#[http(...)]` are proposed annotations consumed
by the contract macro. Types such as `Page<User>` are application-defined examples.

The generated operation identities and client organization would follow the
namespaces:

| Operation | HTTP binding | Illustrative TypeScript call |
| --- | --- | --- |
| `users.get` | `GET /users/{id}` | `api.users.get(id)` |
| `users.list` | `GET /users` with query parameters | `api.users.list(filter)` |
| `posts.create` | `POST /posts` | `api.posts.create(input)` |
| `posts.publish` | `POST /posts/{id}/publish` | `api.posts.publish(id)` |

Rust and TypeScript clients preserve the authored positional method arguments.
Resource composition can later supply accessors
such as `client.users().get(id).await`. The composition API for combining contracts
into an application remains open. Logical input objects remain named in metadata
and are assembled internally by the generated clients.

### Semantics

- A **query** is declared read-only.
- A **mutation** may change state.
- Custom operations such as `publish`, `search`, and `calculate` are first-class.

These classifications are independent of HTTP methods. A complex read-only search
can be bound to POST with a structured body. Caching, invalidation, and retry
behavior need their own policies; a query annotation alone does not establish them.

Operation names and namespaces become wire identifiers. The design needs a way to
preserve those identifiers when Rust items are renamed.

### Explicit bindings and resource conventions

The current preference is explicit HTTP annotations, colocated with operations and
optional in the transport-neutral contract model. This keeps public HTTP behavior
visible and gives tooling enough information to validate mappings.

A possible later `#[resource(...)]` shorthand could expand conventional CRUD
operations into the same binding metadata. Its syntax and defaults are open,
including the distinction between partial updates and full replacement. Resource
organization does not require every contract to implement a fixed CRUD lifecycle.

## Contract metadata and generation

The first [contract implementation](docs/workspace-contracts.md) now generates an
implementable server trait, typed input wrappers, and metadata with directional
input/output/error references. The TypeScript exporter emits an operation map and
typed positional call signatures. The [HTTP binding](docs/http-binding.md) now
adds Axum dispatch and executable Rust/TypeScript clients.

The conceptual outputs are:

```text
Rust trait + DTOs
├── Server trait and typed request types
├── Typed server dispatch
├── Rust client methods
└── Contract metadata + data-type descriptions
    ├── TypeScript types and client
    ├── Documentation
    └── Binding-specific specifications
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

The working preference is deterministic generation through an explicit export
program, with generated artifacts checked for drift. The root workspace and two
generator commands now exercise that workflow; CI integration remains separate.

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

A future WebSocket binding could invoke `posts.create` with its input object. An
in-process binding could invoke the same implementation directly. Both reuse the
operation contract without interpreting HTTP paths or status codes.

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

Axum supplies the first HTTP integration and a Tower middleware boundary.
Operation-level Tower composition can support policies such as timeouts and
concurrency limits; HTTP-specific middleware stays with the HTTP binding.

The precise service request/response types, readiness behavior, and relationship
between application errors and Tower service errors remain research questions.
The generated interface also needs suitable `Send` future guarantees for Axum
while preserving an ergonomic `async fn` authoring experience.

## HTTP binding

Resource-oriented HTTP requires a complete request/response mapping:

- **Inputs:** identify path parameters, query parameters, and the request body.
- **Success:** describe the body and status, including bodyless responses.
- **Declared errors:** map application errors to documented HTTP responses.
- **Response metadata:** provide a path for headers such as `Location` and `ETag`.

For example, the proposed `body = "input"` mapping sends a `NewPost` directly as the
JSON request body. A successful creation returns a `Post` with status 201. The
generated HTTP client knows how to map its named arguments to that request and
decode the response into its typed result.

The server implementation can continue returning `Result<Post, ApiError>`.
Status mappings such as a not-found error becoming 404 belong to the HTTP binding.
Their configuration syntax and defaults are open.

Binding validation should account for missing or conflicting argument mappings,
unsupported path/query shapes, route collisions when composing contracts, and
inconsistent operation semantics such as binding a mutation to GET. Query-string
encoding must be specified so the server, clients, and documentation agree.

## Errors and request context

### Client errors

An implementation declares application outcomes through `Result<T, E>`. A remote
client also encounters transport, protocol, and framework-level failures.

The proposed Rust client shape is:

```rust
Result<User, CallError<ApiError>>
```

The working TypeScript preference is a discriminated result union:

```typescript
type Result<T, E> =
  | { ok: true; value: T }
  | { ok: false; error: E };

// Illustrative generated method signature:
// get(id: UserId): Promise<Result<User, CallError<ApiError>>>;
```

`CallError` is a proposed wrapper whose variants still need definition. The result
union preserves typed error handling; the exact treatment of rejected promises
and unexpected runtime exceptions also needs to be explicit.

Clients therefore have corresponding operation methods with richer failure types
than the server implementation interface. Client-level result wrappers do not
require an identical envelope in every wire binding.

### Request context

Authentication principals, tracing, deadlines, and other server-side context need
to remain separate from serialized operation arguments.

Two approaches remain under consideration:

1. A lightweight, request-scoped implementation holding shared application state
   and the authenticated principal, preserving the authored `&self` trait shape.
2. An explicit context parameter on the generated implementation interface.

The first approach is the current preference to prototype. An authenticated
example should help evaluate ergonomics and middleware integration.

## Spline-owned schema layer

These components have been [promoted into workspace crates](docs/workspace-contracts.md#crate-boundaries).
The research notes below preserve the supporting investigations and earlier API stages.

The first [schema research note](research/schema.md) examines relevant Rust
implementations and records a candidate model, support boundaries, and executable
Serde behavior probes. Its recommendations remain proposals for discussion.

The follow-up [reuse evaluation](research/serde-internals-reuse.md) recommends an
exactly pinned `serde_derive_internals` dependency behind a private macro-side
adapter, supported by behavior comparisons and compiler-failure fixtures.

The [schema/registry prototype](research/schema-registry.md) now supplies a concrete
two-method `Schema` API, directional type registration, field-presence composition,
and explicit custom-codec descriptions. It validates the proposed runtime boundary
with a std-only library and a test-only bridge to the attribute adapter.

The [derive prototype](research/schema-derive.md) now generates descriptions for a
narrow supported surface. It includes logical-name and codec-schema overrides,
direction-specific lowering, recursive/generic DTOs, and diagnostics. A defaulted
`missing_field()` query refines the runtime interface for recursive transparent
wrappers while preserving its two required methods.

The [TypeScript exporter prototype](research/typescript-exporter.md) completes the
first Rust-derive-to-generated-types path. It emits deterministic directional
aliases and references, verifies real Serde samples with TypeScript, and checks
artifact freshness alongside positive and negative type assertions.

The [HTTP implementation](docs/http-binding.md) consumes these schemas for URL
planning, conservative native-client DTO compatibility checks, and generated
TypeScript request/response validation.

Serde remains responsible for serialization and deserialization. Spline's schema
layer would supply the structural descriptions needed by exporters and tooling.

An illustrative derive is:

```rust
#[derive(serde::Serialize, serde::Deserialize, spline::Schema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: UserId,
    pub display_name: String,
}
```

The proposed responsibilities are:

1. **Schema derive:** describe supported structs, enums, and referenced types.
2. **Registry:** collect descriptions, including shared and recursive types.
3. **Serde-aware interpretation:** account for supported representation attributes
   and differences between serialization and deserialization.
4. **Exporters:** generate TypeScript and specification schemas from those
   descriptions.

The important requirement is fidelity to the wire representation. Serde's
`Serialize` and `Deserialize` traits execute logic; they do not provide a complete
static schema interface. Custom implementations can change representations
arbitrarily, so the schema design needs explicit overrides or manual descriptions.

The supported surface needs research: field renaming, enum representations,
defaults, omitted versus null fields, flattening, transparent wrappers, collection
types, large integers, and custom serializers. This is a list to evaluate, not a
claim of initial support. Unsupported cases should produce clear diagnostics.

Schemas also need to distinguish input and output directions. For example, a
TypeScript request must match what the Rust server accepts, while a response type
must describe what the server emits. Serialization-conformance examples should
verify that generated descriptions match actual Serde behavior.

## Documentation and specifications

Capture doc comments and type metadata from the beginning. They can support
operation documentation before standardized specification exporters are complete.

- **OpenAPI:** derive from contract metadata, data schemas, and the HTTP binding.
  RPC-style implementation methods are compatible with resource-oriented HTTP
  documentation. Public API support makes this a valuable follow-on milestone.
- **OpenRPC:** appropriate if a future binding adopts JSON-RPC 2.0 specifically.
- **Neutral tooling:** consume Spline's operation catalog and type descriptions.

The internal metadata model can evolve while the required exports are explored.
Whether to publish and version a standalone Spline specification is an open
question.

## Proposed first implementation milestone

Demonstrate one small API with a query and mutation, an Axum server, and generated
Rust and TypeScript clients working end to end.

Suggested sequence, after the research decisions needed for each step:

1. Define operation identities, supported input shapes, and the first HTTP/JSON
   request, success, and error mappings.
2. Implement a narrow schema model and derives sufficient for the example DTOs.
3. Implement contract expansion, the server interface, typed requests, and metadata.
4. Add the Axum binding and Rust HTTP client.
5. Add deterministic TypeScript export and a small `fetch`-based client.
6. Validate both clients against the same implementation, including declared
   errors, malformed responses, HTTP mappings, and representative Serde shapes.

Verification should cover meaningful compile-time diagnostics, generated
TypeScript type-checking, cross-language request/response behavior, and
deterministic exports. Exact commands depend on the eventual workspace layout.

An in-process binding would then test reuse of the operation/runtime boundary.
OpenAPI export would exercise the completeness of HTTP and schema metadata.

## Research agenda

These are proposed next discussions. Their order and scope should be chosen when
regrouping; they are not implementation commitments.

| Area | Questions to answer | Useful outcome |
| --- | --- | --- |
| Spline-owned schemas | What is the type graph? How are recursive and generic types registered? Which Serde behaviors are supported? How do directional shapes and custom serializers work? | A small schema-model proposal, a Serde support matrix, and representative conformance fixtures. |
| Resource authoring and HTTP | How are contracts composed? Are HTTP annotations explicit or supplied by resource conventions? How do path/query/body mappings, statuses, and headers work? | A few complete API examples with their exact HTTP requests and responses. |
| Errors and request context | What belongs in `CallError`? How do application errors map to HTTP? How are authentication, deadlines, and client call options represented? | Error and context models demonstrated by one authenticated operation. |
| Macro and Tower execution | Which Rust signatures are supported? How are `Send` futures expressed? What are the service types, readiness semantics, and middleware boundaries? | A proposed macro expansion and runtime interface for one operation. |
| Export and evolution | How are artifacts generated and checked? How are client modules packaged? How do operation names, schemas, and HTTP routes evolve compatibly? | An export workflow and explicit compatibility rules. |

The initial schema, derive, TypeScript, and HTTP investigations are implemented
around a [working query/mutation contract](examples/contract/src/lib.rs). The first
profile includes request-scoped context and typed clients, and conservatively
rejects known incompatible shared Rust DTO shapes. Broader asymmetric client
adapters, HTTP response metadata, additional verbs, and declarative status
policies for OpenAPI remain concrete follow-on decisions.

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
