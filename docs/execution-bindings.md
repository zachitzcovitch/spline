# Neutral execution and protocol bindings

Spline contracts and application composition describe operations independently
of their transport. Typed execution uses Tower; protocol bindings own routing,
request context, codecs, binding validation, and transport failures.

```text
Rust traits + DTOs
  ├── neutral contracts, schemas, and resource composition
  └── typed Tower operation services
        ├── in-process calls (owned Rust values)
        └── HTTP binding (JSON + Axum; reqwest/fetch clients)
```

## What the neutral core contains

`spline-contract` owns operation IDs, kinds, argument order, documentation,
input/output/error shapes, and composed resource/schema descriptions. `Operation`
has no HTTP fields. Contract/application construction validates identities and
schema ownership, not route/status/body mappings.

`spline-runtime` depends on the neutral contract model and Tower. The default
facade has no dependency on Axum, reqwest, HTTP binding metadata, or HTTP runtime
libraries. The schema profile remains a Serde-oriented JSON wire-shape model;
neutral execution itself does not serialize values.

```rust
let contract = users::describe()?;
let application = api::describe()?;
```

These descriptions remain available with or without a binding. Optional
`#[http(...)]` annotations can stay colocated with the authored operations. Their
HTTP implementation is available when the facade's `http` feature is enabled.
The feature gate belongs to the facade, so consumers do not need to declare a
matching Cargo feature in their own contract crate.

## Neutral bundles and protocol extensions

Resources and composed APIs also generate a small `Definition` value and a shared
`Services<S>` bundle. `contract()` supplies the neutral contract without an
implementation; `shared(implementation)` stores one implementation in an `Arc`
to reuse across calls. Packing the bundle performs no binding validation and
selects no adapter.

For a reusable implementation, choose the adapter through its extension trait:

```rust
use spline::http::HttpExt;

let router = api::shared(implementation).http(error_statuses)?;
```

For request-scoped HTTP implementations, choose HTTP before supplying the factory:

```rust
use spline::http::HttpFactoryExt;

let router = api::contract().http_with(
    move |context| {
        let state = state.clone();
        async move {
            let principal = authenticate(&context)?;
            Ok(std::sync::Arc::new(Session { state, principal }))
        }
    },
    error_statuses,
)?;
```

The method's HTTP-owned `Fn(RequestContext)` bound provides context inference;
there is no HTTP context type on the neutral definition or bundle. Factory errors
and execution ordering match the lower-level `::http::router_with` API.

For direct typed calls:

```rust
use spline::runtime::InProcessExt;

let client = api::shared(implementation).in_process()?;
let user = client.users().get(id).await?;
```

These methods also work on individual resources, such as `users::shared(...)`.
Clone a bundle to select multiple adapters over the same implementation. Neither
the implementation nor definition needs `Clone`; only their shared handle is cloned.
Adapter selection performs its appropriate validation: in-process selection checks
neutral metadata, while HTTP selection also checks routes, statuses, and body maps.

`HttpExt`/`HttpFactoryExt` belong to `spline-http`, and `InProcessExt` belongs to
the neutral runtime. Generated adapter bridges supply each definition's concrete
service and policy bounds. The extension methods retain compile-time checking of
resource implementations and application-error policies.

## Typed Tower services

Each operation module exposes:

- `Input`: the owned, named argument object.
- `Operation`: implements `spline::runtime::Operation`, with associated input,
  output, and application-error types, logical ID, and query/mutation kind.
- `Service<S>`: a cloneable Tower adapter over `Arc<S>` implementing the authored
  resource trait. Cloning it shares the implementation and does not require `S: Clone`.

For example:

```rust
let service = users::get::Service::new(std::sync::Arc::new(implementation));
let value = spline::runtime::invoke(service, users::get::Input { id }).await?;
```

The base adapter is always ready because the authored `&self` trait has no readiness
method. It returns application results through `Service::Response`:

```text
Service<Input>
  Response = Result<Output, ApplicationError>
  Error = Infallible
```

Outer Tower layers can introduce readiness, backpressure, limits, timeouts, and
operational failures. `invoke` uses Tower's `oneshot`: it waits for readiness and
calls the same service instance that acquired it. It does not clone after acquiring
readiness or introduce retries.

`spline::runtime::CallError<E>` distinguishes `Application(E)` from
`Service(BoxError)`. Keeping application failures in the response preserves their
types through middleware whose operational errors are boxed.

### Configure operation middleware

```rust
use spline::runtime::tower::ServiceExt;

let service = users::get::Service::new(std::sync::Arc::new(implementation))
    .map_request(move |mut input: users::get::Input| {
        // Inspect or transform typed arguments here.
        input.id = canonical_id.clone();
        input
    });
let value = spline::runtime::invoke(service, input).await?;
```

Configure stateful layers on the service once, then clone the configured service
before invocation. Shared middleware state, such as a concurrency limit's semaphore,
can then enforce limits across requests. Enable the appropriate features on your
Tower dependency for layers beyond the re-exported `util` surface.

Dropping an invocation drops its operation future and middleware resources. That
can release concurrency capacity; it does not promise to roll back application
work already performed or stop independently spawned tasks.

## In-process adapter

Select the in-process client through `InProcessExt`, or use the explicit
lower-level `in_process` constructors:

```rust
let client = users::in_process::Client::new(implementation)?;
let user = client.get(id).await?;

let api = api::in_process::Client::new(implementation_of_all_resources)?;
let user = api.users().get(id).await?;
```

Construction validates the neutral description. Calls use owned Rust arguments
and the same typed Tower adapters; there is no URL, HTTP status, serialization,
or request-context factory. Inputs need the neutral contract's decode/schema
requirements and outputs its serialize/schema requirements, but they do not need
the additional bidirectional-codec compatibility required by the native HTTP client.

The convenience client uses the base operation adapters. Use the per-operation
`Service<S>` and `invoke` for customized Tower middleware.

## HTTP selection

The HTTP extensions select the resource/application binding. Lower-level binding
types, metadata, and constructors remain under the `http` module:

```rust
let binding = users::http::describe()?;
let app_binding = api::http::describe()?;
let router = api::http::router_with(session_factory, error_statuses)?;
let client = api::http::Client::new(base_url)?;
```

`spline-http-contract` owns binding descriptions and validation. It is std-only
apart from the neutral schema/contract crates, allowing exporters to use binding
metadata without importing Axum or reqwest. A bound description contains the
neutral description and a separate map of operation IDs to validated HTTP plans.
`ContractBuilder` and `ApplicationBuilder` bind existing neutral descriptions
transactionally and reject missing bindings, conflicting routes, and unsupported
argument/body/status combinations.

`spline-http` owns HTTP execution: Axum routing, request context, decoding, response
envelopes, reqwest calls, and HTTP-specific failures. Factories authenticate/create
the implementation before argument decoding. The handler then creates the neutral
operation adapter, invokes it through Tower, and maps the result into HTTP.

### Bind a configured operation service

The same configured typed service can be bound to its authored HTTP route:

```rust
let router = users::http::get::router(configured_service, error_statuses)?;
```

This route clones the configured service before invoking it, waits for readiness,
decodes arguments, and produces normal HTTP responses. Application failures use
the declared error policy. Service/readiness failures produce a framework 500 with
code `service_failure` rather than masquerading as application errors.

This entry point uses a reusable configured service and requires it to be
`Clone + Send + Sync + 'static`, with a Send future. For request-scoped principals,
use the normal HTTP `router_with` factory or HTTP middleware. Its context is an
HTTP concern; headers/URIs do not appear in neutral operation inputs.

## Exporters

Neutral `export_contract`/`export_application` consume neutral descriptions.
`export_http_client`/`export_http_application` consume the bound descriptions from
`::http::describe()`. HTTP-specific no-content results become `void` only in that
binding's export; the neutral unit shape remains JSON `null`.

## Verification and migration

```sh
npm run check:neutral
cargo run --locked -p spline --no-default-features --example neutral
cargo test --locked -p spline-runtime
cargo test --locked -p spline --no-default-features --test in_process
```

CI checks the no-HTTP normal dependency graph and runs the neutral example, including
a contract with colocated HTTP annotations. Runtime tests cover readiness failures,
reserved service instances, shared concurrency limits, and cancellation. In-process
and HTTP tests apply middleware to typed inputs and preserve typed application errors.

For existing code, change HTTP setup from `api::router_with`/`api::Client` to
`api::http::router_with`/`api::http::Client`, and export HTTP clients from
`api::http::describe()`. Core `describe()` is neutral. Move manual HTTP planning
from core builders to `spline-http-contract` builders over the sealed neutral
description. File/inline module authoring, positional calls, operation IDs, JSON
payloads, and clean TypeScript names retain their authored meaning.
