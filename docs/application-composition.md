# Application composition

Spline can combine resource contracts into one application router and grouped
Rust/TypeScript clients. Enable the facade's `http` feature for the generated
HTTP execution API. `api::describe()` and the in-process adapter remain neutral;
HTTP selection is explicit under `api::http`.

## Declare the application

Define HTTP-bound `Users` and `Posts` traits in ordinary `users` and `posts`
modules using `#[spline::contract]`, then list those resource modules explicitly:

```rust
spline::api! {
    /// The public application API.
    pub mod api {
        users: crate::users,
        posts: crate::posts,
    }
}
```

Each entry is `client_group: resource_module`. The module paths resolve where the
macro is invoked, so `self`, `super`, crate paths, and imported module aliases work.
Group names are unique ASCII identifiers and cannot shadow the Rust client's
constructors, response-policy setter, or `clone`. Raw Rust identifiers are supported.
For a renamed facade, put `crate = "renamed_facade";` before the module declaration.

File-based and inline Rust modules both work. Contracts with an explicit
`module = "requests"` companion can be listed in the same application alongside
same-module contracts; both expose neutral registration and explicit binding modules.

Group names determine client accessors. A contract namespace still determines
operation identity: grouping the `app.users` contract as `people` yields
`client.people().get(id)` in Rust and `client.people.get(id)` in TypeScript, while
the operation ID remains `app.users.get`. The HTTP route remains its declared path.
Dot-separated namespaces do not imply nested client objects or route prefixes.

The generated module exposes:

| Entry point | Purpose |
| --- | --- |
| `api::http::router(service, errors)` | Share one implementation across all HTTP routes. |
| `api::http::router_with(factory, errors)` | Create a request-scoped implementation through one HTTP factory. |
| `api::http::Client::new(base_url)` | Build a grouped native HTTP client. |
| `api::http::Client::with_http(base_url, http)` | Share a configured reqwest transport across groups. |
| `api::describe()` | Collect validated application metadata for export/introspection. |
| `api::http::describe()` | Select and validate HTTP bindings over that description. |
| `api::in_process::Client::new(service)` | Grouped typed calls without transport/serialization. |
| `api::shared(implementation)` | Reuse one implementation across calls. |
| `api::contract()` | Select an adapter whose factory supplies implementations. |

Select an adapter through its extension trait:

```rust
use spline::http::HttpExt;
use spline::runtime::InProcessExt;

let bundle = api::shared(implementation);
let local = bundle.clone().in_process()?;
let routes = bundle.http(error_statuses)?;
```

The cloned bundle shares its implementation across adapters without requiring the
implementation itself to be `Clone`. The extension traits are owned by their
adapters, and selection preserves each contract's concrete bounds.

## Shared state and authentication

The service type implements every listed resource trait. It can hold shared state
directly or delegate to resource-specific implementations. For request-scoped
authentication, use a lightweight session implementing those traits:

```rust
use spline::http::HttpFactoryExt;

let router = api::contract().http_with(
    move |context| {
        let database = database.clone();
        async move {
            let principal = authenticate(&context).await?;
            Ok(std::sync::Arc::new(Session { database, principal }))
        }
    },
    ErrorStatuses,
)?;

axum::serve(listener, router).await?;
```

`authenticate` and `Session` are application code. The factory has the same shape
as the single-resource factory:

```text
Fn(RequestContext) -> Future<Output = Result<Arc<Session>, Fault>>
```

The matched operation invokes it once, before decoding arguments. `RequestContext`
carries method, URI, headers, and cloned middleware extensions. Returning a
`Fault` produces the usual framework error response. Factory rejection prevents
operation dispatch. Authentication and shared state remain outside serialized DTOs.

One error policy is shared across the routes. A function/closure works when all
operations use the same application error; a policy struct can implement
`ErrorPolicy<UserError>` and `ErrorPolicy<PostError>` for different resource errors.
For error types deriving `spline::http::Error`, the reusable `DeclaredErrors` policy
selects their defaults across all resources. `api::http::describe_with(&policy)`
collects the selected policy's response metadata; see [declared errors](http-errors.md).
The service, factory, and policy do not need `Clone`: the generated router shares
them through `Arc`. Each operation keeps its own success and error types.

The returned value is an ordinary Axum router. Add common Tower layers or nest it:

```rust
let router = axum::Router::new().nest("/api", router.layer(common_layer));
```

For that mount, clients use a base URL ending in `/api`. Prefixes and layers are
application configuration; they are not inferred from the resource group names.
Unmatched routes and unsupported HTTP methods use Axum's normal responses and do
not invoke a resource factory.

## Grouped clients

Rust:

```rust
let client = api::http::Client::with_http("http://localhost:3000/api", configured_http)?;
let user = client.users().get("alice".into()).await?;
let post = client.posts().create(NewPost { title: "Hello".into() }).await?;
let published = client.posts().publish(post.id).await?;
```

All accessors borrow the generated resource clients. They share a reqwest client,
the application's operation plans and schema graph, and the base URL. Configure
headers/timeouts on `configured_http`. Calling
`client.with_response_policy(ResponsePolicy::Strict)` applies the policy to every
group; cloning a client retains its configuration. Construction checks the entire
application and each resource's native DTO compatibility.

Generate one TypeScript module:

```rust
let artifact = spline_typescript::Typescript::new()
    .export_http_application(&api::http::describe()?, "Api")?;
```

```typescript
const client = createApiClient("http://localhost:3000/api", {
  headers: { authorization: `Bearer ${token}` },
});

const user = await client.users.get("alice");
const post = await client.posts.create({ title: "Hello" });
if (post.ok) {
  const published = await client.posts.publish(post.value.id);
}
```

One factory initializes the URL, headers, fetch implementation, response policy,
and runtime schema table for all groups. The artifact emits shared DTO definitions
once, a flat `ApiOperations` map keyed by stable operation IDs, and grouped
`ApiCalls<Failure>` signatures. `ApiClient` and `ApiResult` provide clean client and
result types; `ApiHttpOptions` and `ApiHttpFailure` describe the
common HTTP configuration and failures. `export_application` emits only wire types,
the operation map, and grouped signatures, without execution code.

DTOs use short public names and signatures reference them directly. Matching
directions share one declaration; different shapes use clear directional suffixes.
See [TypeScript naming and editor support](typescript.md).

Both clients preserve positional arguments, per-operation application errors,
compatible/strict response decoding, and bodyless 204 behavior.

## Metadata and construction checks

`ApplicationBuilder` registers resource operations into one live schema registry.
The generated resource module exposes `register(&mut builder, group)` for tooling
that wants to compose metadata without generating an HTTP application module:

```rust
let mut builder = spline::contract::ApplicationBuilder::new("Application docs");
users::register(&mut builder, "users")?;
posts::register(&mut builder, "posts")?;
let application = builder.finish()?;
```

`ApplicationDescription` exposes documentation, ordered resource descriptions,
all operations, and one sealed schema graph. Resources retain their namespaces,
docs, and operations. No process-local type identities escape into that graph.

Construction rejects:

- Empty applications/resources and invalid or duplicate group names.
- Duplicate namespaces, even under different client group names.
- Different Rust types claiming the same logical schema identity and direction,
  even if their descriptions have identical shapes. Shared Rust DTOs are reused.
- For HTTP selection, duplicate method/path routes across resources, or conflicting parameter names
  for the same route pattern. Different verbs on exactly the same path are allowed.

Resource registration is transactional. A failed schema or operation check
leaves the builder usable without partially registered definitions. Reversing
resource declaration order produces equal metadata and byte-identical exports.
TypeScript export checks public DTO and utility names across the whole application.
Short-name collisions require an explicit exporter override. Full logical operation
and runtime-schema identities retain their original names.

Neutral composition checks identities and schemas. The separate HTTP binding
description checks routes, body mappings, statuses, and complete bindings before
router construction merges resource routers. A route conflict does not invalidate
the neutral application or its in-process execution. Routes merged manually outside
the declared HTTP application are governed by Axum's own conflict checks.

Both adapters use typed Tower operation services. See [execution and bindings](execution-bindings.md)
for customized middleware, readiness, service failures, and in-process calls.

## Tutorial and executable evidence

Start with the [blog tutorial](../examples/blog/README.md) for a guided application:
separate contracts, server, and Rust client crates alongside a TypeScript package.
`npm run test:blog` builds and launches the actual server and client binaries/scripts.

The [composition fixture](../examples/contract/fixtures/application.rs) defines
`Users`, `Posts`, a shared store, session implementations, and distinct error
policies. The [integration suite](../examples/contract/tests/application.rs) runs
both clients against the same authenticated server, including custom `publish`,
shared user changes visible through posts, errors, 204, nested URLs, response
policy, and construction failures. TypeScript also has static negative assertions.

The [HTTP guide's evidence section](http-binding.md#evidence) records the expanded
Rust suite and blog checks, compiler-failure fixtures, and strict TypeScript
assertions. Verification also covers Clippy with warnings denied, formatting,
neutral facade/dependency checks, and generated-artifact freshness commands.

```sh
cargo test --locked -p spline-example-contract --test application
cargo run --locked -p spline-example-contract -- --check
npm run check
npm run test:blog
```

The composition client is generated into
[`application.generated.ts`](../examples/contract/typescript/application.generated.ts)
by the existing `--write` command. The composition fixtures exercise protocol
and construction edge cases; the blog tutorial provides the introductory walkthrough.
