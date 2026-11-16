# HTTP/JSON binding

Implemented: 2026-10-13.

## What is working

The binding connects a Rust-authored contract to an Axum router, a generated
Rust client backed by reqwest, and a generated TypeScript client backed by fetch.
The [example](../examples/contract/src/lib.rs) implements get, create, replace,
update, and delete over one shared in-memory service. Both clients exercise the
full resource workflow. See [updates and no-content responses](http-updates.md)
for PATCH field semantics and the 204 profile.

The contract remains an operation/type description. HTTP method, path, body
selection, and success status are binding metadata consumed by dispatch and both
clients. Serde performs Rust encoding/decoding; the TypeScript client validates
its serialized arguments and decoded response shapes using the schema graph.

## Explicit entry points and positional calls

```rust
// users.rs, loaded by `pub mod users;`
#[splinekit::contract]
pub trait Users {
    #[query]
    #[http(get = "/users/{id}")]
    async fn get(&self, id: UserId) -> Result<User, ApiError>;

    #[mutation]
    #[http(post = "/users", body = "input", status = 201)]
    async fn create(&self, input: NewUser) -> Result<User, ApiError>;
}
```

The declaration generates support directly in the existing `users` module. Its
namespace defaults to `users`. HTTP usage starts with `users::http::router(...)`
or `users::http::Client::new(...)`; those functions
collect and validate metadata internally. `users::describe()` is the tooling and
introspection entry point for the neutral contract; `users::http::describe()` selects
and validates its separate HTTP mapping.

Inline `pub mod users { ... }` declarations work identically. The optional
`module = "requests"` option creates a separate companion, and `namespace = "v1.users"`
pins operation identities when needed. See [contract identity and options](workspace-contracts.md#identity-and-options).

The TypeScript exporter now preserves authored argument order and positional
arity, matching the Rust client:

```rust
client.get(id).await?;
client.create(new_user).await?;
```

```typescript
await client.get(id);
await client.create(newUser);
```

An operation still has a named logical input object for metadata and dispatch.
That object is assembled inside the client. `create(newUser)` sends `newUser`
directly as its HTTP body because of `body = "input"`.

`Operation.arguments` stores declaration order separately from sorted operation
maps. TypeScript parameters are required positional parameters, including nullable
Option arguments. A structured DTO can itself have optional/defaulted properties.
Argument names still determine logical field names and default query keys, so
renaming a parameter can change the wire contract even when its call position stays
the same.

## HTTP annotation and argument mapping

The annotation accepts one of `get`, `post`, `put`, `patch`, or `delete` with a
path, a `body = "argument"` mapping where applicable, and `status = 200`, `201`,
or `204` (default 200).

| Input location | Rule |
| --- | --- |
| Path | A whole segment such as `{id}` selects the argument of that name. |
| JSON body | `body = "input"` selects one argument's JSON value directly. |
| Query | Remaining arguments become query parameters under their argument names. |

GET is permitted only for queries and has no body. POST can bind either kind,
including read-only searches. PUT/PATCH/DELETE must be mutations; PUT and PATCH
require a body mapping, and DELETE is bodyless in this profile. Every operation
in a selected HTTP contract needs a binding. Incomplete mappings do not invalidate
neutral composition or in-process calls; selecting `::http::describe()` returns a
missing-binding error. The base `splinekit` crate remains
usable without its optional HTTP features for neutral contracts and typed execution.

### Client, server, or both

| Facade feature | Generates | Depends on |
| --- | --- | --- |
| `http-client` | `::http::Client` and descriptions | reqwest; builds for `wasm32-unknown-unknown` |
| `http-server` | `::http::router`/`router_with` and descriptions | Axum |
| `http` | both | both |

Choose per crate. A contracts crate shared by a server and a client usually
enables `http-client`; the server crate adds `http-server` (or `http`). Cargo
unifies features per build, so the server's build generates the router while a
browser build of the client never compiles Axum. The blog example is laid out
this way, and CI checks its contracts crate for `wasm32-unknown-unknown`. On
wasm32 the reqwest client uses the browser's `fetch`, so redirect handling is the
browser's.

Path/query arguments support strings, booleans, the evaluated fixed-width integer
types, and compatible string-literal unions. Optional nullable query values are
omitted when null; required query values must be supplied. Paths require
non-nullable, required scalar arguments. Path values are encoded as individual
segments; empty values and complete `.`/`..` segments are rejected by the clients.

Query strings use URL form encoding. The server rejects duplicate or unknown
query keys, invalid boolean/integer values, and out-of-range integers. Complex
query objects, array/repeated query conventions, wildcard paths, and body/path
argument overlap are outside this initial profile.

Binding validation checks argument mappings, scalar shapes, and ambiguous route
patterns before router registration. The same dynamic route with different
placeholder names is rejected, including when the HTTP verbs differ, because
Axum cannot merge those patterns safely. [`splinekit::api!`](application-composition.md)
validates these conflicts across all declared resources before merging routers.
Additional routes merged manually outside that application remain governed by Axum.

## Wire responses

For 200/201, success is unwrapped JSON with the declared status and
`Content-Type: application/json`. A successful POST creation looks like:

```http
HTTP/1.1 201 Created
Content-Type: application/json

{"id":"user_2","displayName":"New user","bio":null}
```

Errors are [RFC 9457](https://www.rfc-editor.org/rfc/rfc9457) problem details,
machine-first: every error body has a numeric `status` matching the HTTP status
and a string `code`. Clients branch on `code` and the fields beside it. Nothing in
an error is text meant for end users; what a person reads is the application's
choice.

An application error is the error value itself, an object tagged by `code`
(`#[serde(tag = "code")]`), with `status` added. Its HTTP status comes from the
server's error policy:

```http
HTTP/1.1 404 Not Found
Content-Type: application/problem+json

{"status":404,"code":"not_found"}
```

Framework faults (malformed input, a rejected request factory, a failed service)
add an optional `detail`: developer-facing text for logs and debugging, which may
change between versions:

```http
HTTP/1.1 400 Bad Request
Content-Type: application/problem+json

{"status":400,"code":"invalid_request","detail":"..."}
```

No header marks either kind, and nothing on the wire names Splinekit. A client
recognizes an application error by its body: a `code` the operation declares at
that status, which must then decode as that error. Any other problem body is a
framework fault with its `code`; anything that isn't problem details from the
service (a proxy's page, a gateway's JSON) is an unrecognized HTTP failure.

A gateway's JSON can look like problem details, so a contract should say where
its problem types live, on the trait:

```rust
#[splinekit::contract]
#[http(problem_types = "https://example.com/errors")]
pub trait Users { /* ... */ }
```

Then every error also carries RFC 9457's `type`: the base, a slash, and the code
with underscores as hyphens.

```http
HTTP/1.1 400 Bad Request
Content-Type: application/problem+json

{"type":"https://example.com/errors/invalid-request","status":400,"code":"invalid_request","detail":"..."}
```

Clients generated from the contract, in Rust and TypeScript, accept problem
details only when `type` is the service's own for that `code`; anything else is an
unrecognized HTTP failure. Without `problem_types`, a body with a matching
`status` and a string `code` counts as the service's. The base must be an absolute
URI without a query, fragment or trailing slash, and every contract in one
`api!` application must declare the same base, since one client serves them all.
Routes render a request factory's `Fault` with the type too; a `Fault` turned into
a response anywhere else (`IntoResponse`) has none.

To keep application errors and faults apart, binding checks each operation's error
type:

- it serializes as objects (or a union of objects) with a required literal `code`;
- no `code` is one the binding's own faults use: `invalid_request`,
  `body_too_large`, `unsupported_media_type`, `internal`, `service_failure`;
- no field is named like an RFC 9457 member (`type`, `title`, `status`, `detail`,
  `instance`).

A request factory's faults use codes the application chooses, such as
`unauthorized`; don't reuse a code the operation declares at the same status.

JSON request bodies accept `application/json` and application subtypes ending in
`+json`, including content-type parameters. A missing/wrong media type receives
415, invalid JSON/DTO/query input receives 400, and the fixed 2 MiB request-body
limit receives 413. Bodyless operations reject nonempty bodies. Default unmatched
routes and method rejections remain Axum's responses and are classified by clients
as unmarked HTTP errors.

A declared 204 is an empty success response and requires a unit result. Clients
return `()` in Rust and `undefined`/`void` in TypeScript, with no JSON parsing.
Application and framework errors still carry JSON. See the [204 rules](http-updates.md#204-is-distinct-from-json-null)
for validation and the distinction from a 200 JSON-null result. Streaming,
additional status codes, and per-operation response headers remain follow-on work.

## Server and request context

Enable `splinekit`'s `http` Cargo feature, then:

```rust
let router = users::http::router(DemoUsers::default(), error_status)?;
axum::serve(listener, router).await?;
```

The protocol-owned extension form is:

```rust
use splinekit::http::HttpExt;
let router = users::shared(DemoUsers::default()).http(error_status)?;
```

For a request-scoped implementation, import `HttpFactoryExt` and call
`users::contract().http_with(factory, error_status)?`. The factory's context type
is inferred at the HTTP method, keeping neutral bundles independent of headers/URIs.

For the example, `error_status(&ApiError)` returns 404 for `NotFound` and 422 for
`InvalidName`. The policy must return a 4xx/5xx status. Invalid policies and response
serialization failures produce a framework 500. A closure implements
`ErrorPolicy<E>`; a custom policy type can implement the trait for several error
types when a contract uses different errors per operation.

Error types can opt into `#[derive(splinekit::http::Error)]` with
`#[http(status = StatusCode::NOT_FOUND)]` on each variant. `DeclaredErrors` selects
these defaults, and `::http::describe_with(&policy)` collects their per-status
response shapes. Custom executable policies remain available and opaque unless
they supply static mappings. See [declared HTTP errors](http-errors.md) for authoring,
validation. [OpenAPI export](openapi.md) consumes this metadata and combines
application/framework alternatives where they share a status.

`users::http::router_with(factory, error_policy)` adds an HTTP request-scoped factory:

```text
Fn(RequestContext)
    -> Future<Output = Result<Arc<Implementation>, Fault>>
```

`RequestContext` owns the request method, URI, headers, and cloned extensions.
Middleware can attach authentication data to extensions. The factory may build a
lightweight implementation holding shared state and that principal, or return a
framework fault such as 401. It runs before argument decoding and invocation.
The API trait and its serialized parameters contain none of this transport state.

The normal router convenience function shares one implementation through `Arc`.
Both entry points return an ordinary Axum `Router`, supporting its Tower layers
and route nesting. Tests cover headers, middleware extensions, factory rejection,
and a client configured to satisfy that request-scoped policy.

For several resources, `api::http::router_with` shares this factory and error policy
across the whole application. The returned session implements each resource trait.
See [shared state and authentication](application-composition.md#shared-state-and-authentication).

## Rust client

```rust
let client = users::http::Client::new("http://127.0.0.1:3000")?;
let user = client.get(UserId("user_1".into())).await?;
let created = client.create(new_user).await?;
```

`Client::with_http(base, reqwest_client)` accepts a configured reqwest client for
default headers, timeouts, and other transport policies. The default constructor
disables redirects. No automatic retries are added by Splinekit.

### Transports and layers

`Client::with_transport(base, transport)` sends through any Tower service from
`splinekit::http::HttpRequest` (an `http::Request<Bytes>`) to `HttpResponse`.
`ReqwestTransport` is the default. Authentication, session refresh, retries and
timeouts are layers around it, written once for every generated client:

```rust
// Send the session token; on 401, refresh it once and re-send the same request.
let transport = RefreshOnce { inner: ReqwestTransport::new(reqwest::Client::new()), token };
let client = users::http::Client::with_transport("https://api.example.com", transport)?;
```

The request body is `Bytes`, so a layer can re-send it without copying. A
transport must be `Clone`; on native targets it and its futures must also be
`Send` (and `Sync`), on wasm32 they needn't be, since browser futures aren't.
`examples/contract/tests/http.rs` has a complete refresh-once layer. Whether a
request is safe to repeat is the application's call: the client sends each
generated call exactly once.

The generated methods return `Result<Output, splinekit::http::CallError<ApiError>>`:

| Variant | Meaning |
| --- | --- |
| `Application(E)` | A decoded, declared application error. |
| `Framework(Fault)` | A framework fault (or an error code this client doesn't declare), with its status, code and optional developer `detail`. |
| `Http { status }` | An error response that isn't problem details from the service. |
| `Transport(BoxError)` | The transport failed to send the request or read the response. |
| `Protocol(String)` | Unexpected success status or headers, invalid response data, or a declared error code whose body doesn't decode. |
| `Encode(String)` | Arguments could not be encoded for their binding. |

Native clients now validate response JSON against the known output/error shape
before Serde decoding. The default `ResponsePolicy::Compatible` projects closed
objects onto known fields so added response properties do not break strict Rust
DTOs. `client.with_response_policy(ResponsePolicy::Strict)` enforces the recorded
unknown-field rules. Known fields and enum variants stay validated in both modes.

HTTP contracts generate serializable as well as deserializable input wrappers.
Native client methods also require success/error DTO deserialization. Thus this
first shared-Rust-client profile requires bidirectional Serde DTOs.

Construction conservatively checks supported serialized shapes against their
deserialized counterparts. Directional names that make a type reject its own JSON
produce an early build error. The check permits ordinary defaults/optional inputs
and narrower emitted shapes; it does not prove custom serializer code, format
predicates, or preservation of every nested Option state. Asymmetric DTOs may
still need explicit client adapters in a later profile.

## TypeScript client

The exporter has separate entry points:

- `export_contract(...)`: wire types, metadata types, and positional signatures.
- `export_http_client(...)`: those declarations plus an executable fetch client.
- `export_application(...)` / `export_http_application(...)`: shared types and
  grouped signatures/executable clients for several resources.

HTTP exporters receive the bound description from `::http::describe()`. Neutral
exporters receive `::describe()`. HTTP metadata and validation live in the lightweight
`splinekit-http-contract` crate, while Axum/reqwest execution remains in `splinekit-http`.
Dispatch invokes the same typed Tower services used in process. See
[execution and bindings](execution-bindings.md) for the protocol boundary and
per-operation HTTP routers over configured middleware.

```typescript
const client = createUsersClient("http://127.0.0.1:3000", {
  headers: { "x-example": "value" },
});
const result = await client.create({ displayName: "TypeScript client" });
if (result.ok) {
  console.log(result.value.id);
} else if (result.error.kind === "application") {
  console.log(result.error.value.code);
}
```

An injected `fetch` implementation is also supported. Operation failures resolve
to typed result unions; URL/header configuration errors can throw when the client
is constructed. The failure union includes application, framework, HTTP,
transport, protocol, and encode categories. The default fetch binding uses manual
redirect handling.

The injected `fetch` is where an application adds the same controls as a Rust
transport layer: a session token, one refresh on 401, retries, timeouts:

```typescript
let token = await getSession();
const client = createUsersClient(base, {
  fetch: async (url, init) => {
    const send = (t: string) => {
      const headers = new Headers(init?.headers);
      headers.set("authorization", `Bearer ${t}`);
      return fetch(url, { ...init, headers });
    };
    const response = await send(token);
    if (response.status !== 401) return response;
    token = await refreshSession();
    return send(token); // the body is a string, so it can be sent again
  },
});
```

The generated client touches no browser-only globals and returns plain JSON data
(strings for dates, decimals and 64-bit integers), so it works the same in
browsers, Node and React Server Components, and results can cross a
server/client boundary as they are.

Generated DTOs use clean public names such as `User`, `NewUser`, and `ApiError`.
Signatures reference those types directly and preserve normal argument names.
`UsersClient` names the HTTP client and `UsersResult` names its typed results.
Differing input/output shapes retain directional suffixes. See
[TypeScript types and editor support](typescript.md) for the naming rules and
language-service checks.

The emitted structural validator checks JSON-encoded arguments against the authored
input schema. Response decoding separately defaults to compatible projection of
known fields; set `responsePolicy: "strict"` in client options to enforce the
recorded unknown-field policy. Both response modes validate references, unions,
recursive containers, required fields, nullability, supported integer ranges,
finite floats, and [formatted strings](wire-types.md) (UUID, date-time, date, URI,
decimal and range-checked integer digits).
Response projection has a traversal-depth limit of 256. Schema-declared custom
predicates remain outside that structural check. See [contract evolution](contract-evolution.md)
for the cross-version policy and tests.

Both clients accept an HTTP(S) base URL with an optional path prefix and reject
credentials, query strings, and fragments in that base. Headers carry credentials
when needed. URL path/query encoding and nested base prefixes are exercised in
the conformance suite.

## Run the example

In one terminal:

```sh
cargo run --locked -p splinekit-example-contract --bin server -- 127.0.0.1:3000
```

In another:

```sh
cargo run --locked -p splinekit-example-contract --bin rust-client -- http://127.0.0.1:3000
node examples/contract/typescript/client.mjs http://127.0.0.1:3000
```

The Node runner executes the generated TypeScript through built-in type stripping
(tested on Node 25.2.1). Browser/bundler users import the same generated module.

## Evidence

The Rust suite and blog checks cover **184 behavior/integration checks**, plus trybuild harnesses
with **35 compiler-failure fixtures** when HTTP is enabled. The eight live HTTP tests include both
client executables calling one loopback server, Rust/TypeScript path and query
round trips, request context, declared error statuses, malformed requests,
protocol failures, transport failures, and native DTO compatibility rejection.
Six additional evolution tests compare original/newer servers using clients built
from a fixed older contract, including live TypeScript execution.
Four resource-update tests cover PUT/PATCH/DELETE, atomic application validation,
and bodyless responses, alongside the generic `Patch<T>` tests.
Nine application-composition tests exercise grouped clients, shared authentication
and state, resource-specific errors, common response policies, and construction
failures, mixed binding composition, and in-process execution. Four core
application-builder tests, separate HTTP binding tests, and four exporter tests cover shared
schema ownership, rollback, cross-resource routes, deterministic output, and naming.
The [blog tutorial](../examples/blog/README.md) adds a separate `npm run test:blog`
smoke test. It builds the server and Rust client crates, launches the actual server
and both client entry points, then verifies their persisted effects and application
validation over HTTP.
The original Serde inspection and wire probes contribute 29 additional behavior
tests and three of the compiler-failure fixtures, now included in the root suite.
Typed runtime tests cover readiness, operational failures, backpressure across
clones, and cancellation. Neutral-only tests verify in-process operation middleware
and contracts with incomplete optional HTTP mappings.
Declared-error checks compare real responses with per-status problem schemas,
including payload variants, Serde renames, shared statuses, and custom overrides.
OpenAPI checks independently validate documents, Serde samples, negative instances,
and live responses, including nullable bodies, omitted queries, PATCH, and 204.

The TypeScript workspaces pass strict compilation with **61 expected-error assertions**.
Runtime Node assertions additionally exercise generated request/response validation,
typed errors, injectable fetch behavior, and direct JSON body selection. Cargo
formatting, Clippy with warnings denied, neutral facade/dependency checking, and
generated-artifact freshness pass.

```sh
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo check --locked -p splinekit --no-default-features
cargo run --locked -p splinekit-typescript --example export -- --check
cargo run --locked -p splinekit-example-contract -- --check
cargo run --locked -p splinekit-example-blog-contracts --example blog-export -- --check
npm ci --ignore-scripts --no-audit --no-fund
npm run check
npm run test:blog
```

The HTTP integration tests need loopback networking and a Node runtime with
TypeScript stripping. The lockfiles record the evaluated versions, including
TypeScript 5.9.3, Axum 0.8.9, and reqwest 0.12.28; the crate manifests accept
compatible later releases.
