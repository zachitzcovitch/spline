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
#[spline::contract]
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
namespace defaults to `users`. Ordinary usage starts with `users::router(...)`
or `users::Client::new(...)`; those functions
collect and validate metadata internally. `users::describe()` is the tooling and
introspection entry point.

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
in an HTTP-annotated contract needs a binding. The base `spline` crate remains
usable without its optional `http` feature for schema/metadata-only contracts.

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
Axum cannot merge those patterns safely. [`spline::api!`](application-composition.md)
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

An application error uses the status supplied by the server's error policy:

```http
HTTP/1.1 404 Not Found
Content-Type: application/json
x-spline-error: application

{"kind":"application","value":{"kind":"not_found"}}
```

Framework failures, including malformed input and request-factory rejection, use:

```http
HTTP/1.1 400 Bad Request
Content-Type: application/json
x-spline-error: framework

{"kind":"framework","code":"invalid_request","message":"..."}
```

The marker header and envelope kind must agree. This distinguishes declared
application errors from framework failures and unmarked proxy/server responses.
For cross-origin browser use, the application's CORS layer must allow the desired
origins/headers and expose `x-spline-error` to fetch.

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

Enable `spline`'s `http` Cargo feature, then:

```rust
let router = users::router(DemoUsers::default(), error_status)?;
axum::serve(listener, router).await?;
```

For the example, `error_status(&ApiError)` returns 404 for `NotFound` and 422 for
`InvalidName`. The policy must return a 4xx/5xx status. Invalid policies and response
serialization failures produce a framework 500. A closure implements
`ErrorPolicy<E>`; a custom policy type can implement the trait for several error
types when a contract uses different errors per operation.

The status policy is executable server configuration. A future complete OpenAPI
export will need declarative metadata for the policy's possible status mappings.

`users::router_with(factory, error_policy)` adds a request-scoped service factory:

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

For several resources, `api::router_with` shares this factory and error policy
across the whole application. The returned session implements each resource trait.
See [shared state and authentication](application-composition.md#shared-state-and-authentication).

## Rust client

```rust
let client = users::Client::new("http://127.0.0.1:3000")?;
let user = client.get(UserId("user_1".into())).await?;
let created = client.create(new_user).await?;
```

`Client::with_http(base, reqwest_client)` accepts a configured reqwest client for
default headers, timeouts, and other transport policies. The default constructor
disables redirects. No automatic retries are added by Spline.

The generated methods return `Result<Output, spline::http::CallError<ApiError>>`:

| Variant | Meaning |
| --- | --- |
| `Application(E)` | A decoded, declared application error. |
| `Framework(Fault)` | A structured framework failure, retaining status/code/message. |
| `Http { status }` | An unmarked/unrecognized HTTP failure response. |
| `Transport(reqwest::Error)` | Network or response-body transport failure. |
| `Protocol(String)` | Unexpected success status, invalid headers/envelope, or invalid response data. |
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

```typescript
const client = createUsersClient("http://127.0.0.1:3000", {
  headers: { "x-example": "value" },
});
const result = await client.create({ displayName: "TypeScript client" });
if (result.ok) {
  console.log(result.value.id);
} else if (result.error.kind === "application") {
  console.log(result.error.value.kind);
}
```

An injected `fetch` implementation is also supported. Operation failures resolve
to typed result unions; URL/header configuration errors can throw when the client
is constructed. The failure union includes application, framework, HTTP,
transport, protocol, and encode categories. The default fetch binding uses manual
redirect handling.

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
recursive containers, required fields, nullability, and supported integer ranges.
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
cargo run --locked -p spline-example-contract --bin server -- 127.0.0.1:3000
```

In another:

```sh
cargo run --locked -p spline-example-contract --bin rust-client -- http://127.0.0.1:3000
node examples/contract/typescript/client.mjs http://127.0.0.1:3000
```

The Node runner executes the generated TypeScript through built-in type stripping
(tested on Node 25.2.1). Browser/bundler users import the same generated module.

## Evidence

The Rust suite and blog checks cover **159 behavior/integration checks**, plus trybuild harnesses
with **27 compiler-failure fixtures** when HTTP is enabled. The seven live HTTP tests include both
client executables calling one loopback server, Rust/TypeScript path and query
round trips, request context, declared error statuses, malformed requests,
protocol failures, transport failures, and native DTO compatibility rejection.
Six additional evolution tests compare original/newer servers using clients built
from a fixed older contract, including live TypeScript execution.
Four resource-update tests cover PUT/PATCH/DELETE, atomic application validation,
and bodyless responses, alongside the generic `Patch<T>` tests.
Five application-composition tests exercise grouped clients, shared authentication
and state, resource-specific errors, common response policies, and construction
failures. Five core application-builder and four exporter tests cover shared
schema ownership, rollback, cross-resource routes, deterministic output, and naming.
The [blog tutorial](../examples/blog/README.md) adds a separate `npm run test:blog`
smoke test. It builds the server and Rust client crates, launches the actual server
and both client entry points, then verifies their persisted effects and application
validation over HTTP.
The original Serde inspection and wire probes contribute 29 additional behavior
tests and three of the compiler-failure fixtures, now included in the root suite.

The TypeScript workspaces pass strict compilation with **61 expected-error assertions**.
Runtime Node assertions additionally exercise generated request/response validation,
typed errors, injectable fetch behavior, and direct JSON body selection. Cargo
formatting, Clippy with warnings denied, metadata-only feature checking, and
generated-artifact freshness pass.

```sh
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo check --locked -p spline --no-default-features
cargo run --locked -p spline-typescript --example export -- --check
cargo run --locked -p spline-example-contract -- --check
cargo run --locked -p spline-example-blog-contracts --example blog-export -- --check
npm ci --ignore-scripts --no-audit --no-fund
npm run check
npm run test:blog
```

The HTTP integration tests need loopback networking and a Node runtime with
TypeScript stripping. TypeScript 5.9.3, Axum 0.8.9, and reqwest 0.12.28 are pinned;
the lockfiles record the remaining evaluated dependencies.
