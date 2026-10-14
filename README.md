# Spline

A Rust-first contract system for servers and clients. Define your operations and
wire types once in Rust; implement the server traits and consume the same contract
through typed Rust and TypeScript clients.

Spline combines Serde serialization, a directional schema graph, typed Tower
execution, and generated clients with typed application errors. Resource contracts
compose into a neutral application; the HTTP binding adds Axum routing, shared
request context, and client configuration.

[Tutorial](examples/blog/README.md) · [HTTP guide](docs/http-binding.md) ·
[Contributing](CONTRIBUTING.md) · [Roadmap](ROADMAP.md) · [Changelog](CHANGELOG.md) ·
[Benchmarks](benchmarks/README.md) ·
[CI](https://github.com/zachitzcovitch/spline/actions/workflows/ci.yml)

**Status: experimental.** The implemented profile is exercised by real Rust and
TypeScript clients in CI. APIs may evolve between commits; crates are currently
unpublished. Start from this checkout using the pinned Rust and Node toolchains.

## Start with the blog tutorial

The [blog example](examples/blog/README.md) walks through a small monorepo with
separate contract, server, and Rust client crates plus a TypeScript package.
It has shared authentication, an in-memory store, grouped `Users`/`Posts` calls,
and typed error handling, with exact commands and a guided reading order.

## Define a resource contract

Declare the contract in a normal Rust resource module, for example `users.rs`:

```rust
#[spline::contract]
pub trait Users {
    /// Fetch an existing user.
    #[query]
    #[http(get = "/users/{id}")]
    async fn get(&self, id: UserId) -> Result<User, ApiError>;

    /// Create a user from the supplied profile.
    #[mutation]
    #[http(post = "/users", body = "input", status = 201)]
    async fn create(&self, input: NewUser) -> Result<User, ApiError>;
}
```

DTOs use Serde and `#[derive(spline::Schema)]`. The macro generates:

- An implementable `Users: Send + Sync` trait with `Send` operation futures.
- Typed argument structs and HTTP support in the existing `users` module.
- Typed Tower operation services and `users::in_process::Client` for direct calls.
- `users::http::router(service, error_status)` and `users::http::Client::new(base_url)`
  when HTTP is selected.
- `users::describe()` for export tooling and introspection. Server/client setup
  collects the metadata internally.

The namespace defaults to the resource module name, so this gives `users.get` and
`users.create`. An inline `pub mod users { ... }` works the same way. Optional
`namespace` and `module` settings pin an identity or create a separate companion
when needed. The [blog tutorial](examples/blog/README.md) demonstrates the defaults.

Export the metadata:

```rust
let contract = users::http::describe()?;
let artifact = spline_typescript::Typescript::new()
    .export_http_client(&contract, "Users")?;
```

The [generated TypeScript](examples/contract/typescript/generated.ts) includes
`createUsersClient`, typed results, and structural JSON validation. Both clients
preserve authored positional parameters:

```rust
let client = users::http::Client::new("http://localhost:3000")?;
let user = client.get(UserId("user_1".into())).await?;
```

```typescript
const client = createUsersClient("http://localhost:3000");
const user = await client.get("user_1");
const created = await client.create({ displayName: "New user" });
if (created.ok) {
  console.log(created.value.id);
} else if (created.error.kind === "application") {
  console.log(created.error.value.kind);
}
```

Enable the `spline` crate's `http` feature for bound contracts. The binding supports
GET, POST, PUT, PATCH, and DELETE, scalar path/query parameters, selected JSON
bodies, 200/201 JSON successes, and empty 204 successes. See the
[HTTP guide](docs/http-binding.md) for the wire format, request-context factory,
error policies, and supported DTO compatibility.

## Supported profile

| Area | Current support |
| --- | --- |
| Authoring | Owned DTOs, Serde derives, safe non-generic resource traits, queries and mutations. |
| Schemas | Directional names/presence, tagged enums, generic and recursive types, explicit custom-codec descriptions. |
| HTTP | GET/POST/PUT/PATCH/DELETE, scalar path/query arguments, selected JSON bodies, 200/201/204 successes. |
| Clients | Positional calls, typed errors, shared configuration, compatible or strict response decoding. |
| Execution | Typed Tower operation services and positional in-process Rust clients. |
| Composition | Neutral resource/schema composition; grouped HTTP clients and one router with shared request-scoped services. |

The evaluated numeric JSON profile uses 8/16/32-bit integers. Larger integers and
floats need an explicit wire representation. Error types can opt into
[`spline::http::Error`](docs/http-errors.md) to declare default HTTP statuses and
documented response shapes. Streaming and OpenAPI are [follow-up work](ROADMAP.md).
Each guide documents the supported surface and its boundaries.

Partial-update DTOs use `Patch<T>` to preserve omission separately from a value.
For example, `Patch<Option<String>>` distinguishes an unchanged bio from clearing
it with null or replacing it with a string. See [update semantics](docs/http-updates.md)
for the Serde annotations and matching TypeScript calls.

Clients default to compatible response decoding: known fields stay validated,
while additive properties of closed response objects are projected away. Use
`ResponsePolicy::Strict` in Rust or `responsePolicy: "strict"` in TypeScript for
exact schema checks. [Contract evolution](docs/contract-evolution.md) documents
the old-client/new-server guarantees and their boundaries.

## Compose resources into an application

List the resource modules once:

```rust
spline::api! {
    pub mod api {
        users: crate::users,
        posts: crate::posts,
    }
}

use spline::http::HttpFactoryExt;
let router = api::contract().http_with(session_factory, error_policy)?;
let client = api::http::Client::new("http://localhost:3000")?;
let user = client.users().get(id).await?;
```

`Typescript::export_http_application(&api::http::describe()?, "Api")` generates one
`createApiClient` factory with `client.users.get(id)` and `client.posts.create(input)`.
Resources share configuration and schemas. Neutral composition validates
namespaces and schema identities; HTTP selection validates routes across the API. See
[application composition](docs/application-composition.md) for shared authentication,
state, error policies, and executable Rust/TypeScript fixtures.

`api::describe()` remains neutral. `api::in_process::Client::new(implementation)?`
uses the same typed Tower operation services with direct Rust values. HTTP route,
body, and status validation occurs in the separately selected HTTP binding. See
[execution and bindings](docs/execution-bindings.md) for Tower middleware, readiness,
and protocol boundaries.

For shared implementations, import `spline::http::HttpExt` and select HTTP with
`api::shared(implementation).http(error_policy)?`. Import
`spline::runtime::InProcessExt` to select `.in_process()?` on that same neutral
bundle. Adapter-specific methods become available through the corresponding trait.

## Workspace

| Crate | Responsibility |
| --- | --- |
| `spline` | User-facing facade, macros, and generated-code integration. |
| `spline-schema` | Schema trait, directional graph, and registry. |
| `spline-contract` | Operation metadata and transactional contract construction. |
| `spline-macros` | Schema derive and contract attribute; private Serde adapter. |
| `spline-typescript` | Deterministic wire types and operation signatures. |
| `spline-http` | Axum dispatch support, HTTP errors, and the reqwest client runtime. |
| `spline-http-contract` | Lightweight HTTP binding descriptions and validation. |
| `spline-runtime` | Protocol-neutral typed Tower execution and service failures. |

`examples/blog` is the guided multi-crate application tutorial. `examples/contract` contains
the CRUD example and protocol/evolution/composition conformance fixtures. Packages
are currently unpublished.

## Development setup

Rust and Node versions are pinned in `rust-toolchain.toml` and `.nvmrc`. With
rustup and nvm installed, prepare a checkout from the repository root:

```sh
nvm install
nvm use
rustup show
npm ci --ignore-scripts --no-audit --no-fund
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for focused checks, generated artifacts, and
the repository layout. [CI](.github/workflows/ci.yml) runs the verification suite
on pushes and pull requests.

## Run the server and clients

Start the server in one terminal:

```sh
cargo run --locked -p spline-example-blog-server
```

Then run either client:

```sh
cargo run --locked -p spline-example-blog-client
node examples/blog/typescript/run.mjs
```

The Node runner imports the generated TypeScript directly using built-in type
stripping; Node 25.2.1 is the tested runtime. Both clients read a profile, create a
draft, handle a typed authorization error, and publish the post. See the
[walkthrough](examples/blog/README.md) for output, configuration, and the source tour.

## Run and verify

From the repository root:

```sh
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo check --locked -p spline --no-default-features
npm run check:neutral
cargo run --locked -p spline --no-default-features --example neutral
cargo run --locked -p spline-typescript --example export -- --check
cargo run --locked -p spline-example-contract -- --check
cargo run --locked -p spline-example-blog-contracts --example blog-export -- --check
npm ci --ignore-scripts --no-audit --no-fund
npm run check
npm run test:blog
```

Cargo and npm dependencies are locked at the workspace root. TypeScript 5.9.3 is
pinned, with strict checks, exact optional properties, and checked index access.
The HTTP integration suite starts loopback servers and launches the Rust and Node
clients, so a Node runtime with built-in TypeScript stripping is needed for the
complete workspace test run.

## Performance

[The benchmark suite](benchmarks/README.md) measures schema/contract construction,
TypeScript export, native loopback HTTP calls, and isolated generated TypeScript
client processing. It documents the fixtures and timing boundaries so you can
build local baselines and compare changes:

```sh
cargo bench --locked -p spline-benchmarks
npm run bench:typescript
```

CI checks benchmark correctness; timing comparisons use local baselines on the
same machine and toolchain.

## Design and research

- [Workspace and contract milestone](docs/workspace-contracts.md): implemented API,
  crate boundaries, verification, and current scope.
- [First HTTP binding](docs/http-binding.md): authoring, server/client examples,
  wire behavior, request context, and conformance checks.
- [Declared HTTP errors](docs/http-errors.md): derived default statuses, policy-aware
  response metadata, and custom policy overrides.
- [Updates and no-content responses](docs/http-updates.md): PUT/PATCH/DELETE,
  explicit missing/null/value semantics, and 204 handling.
- [Contract evolution](docs/contract-evolution.md): additive changes, response
  policies, closed enums, and versioned Rust/TypeScript conformance fixtures.
- [Application composition](docs/application-composition.md): grouped clients,
  shared state/authentication, and whole-application validation.
- [TypeScript types and editor support](docs/typescript.md): readable public names,
  direct signatures, directional differences, and collision overrides.
- [Execution and bindings](docs/execution-bindings.md): neutral composition, typed
  Tower services, in-process calls, and explicit HTTP selection.
- [Overall design](DESIGN.md): agreed direction and the broader roadmap.
- [Research index](research/README.md): the experiments and source reviews behind
   the implementation, including where their code and tests now live.

## Contributing

Bug reports, documentation improvements, API feedback, and focused pull requests
are welcome. Try the blog tutorial, browse the [roadmap](ROADMAP.md), and read
[CONTRIBUTING.md](CONTRIBUTING.md) for the development loop and generated artifacts.
For a bug, include a small contract or request that reproduces it and your toolchain
versions in an [issue](https://github.com/zachitzcovitch/spline/issues/new/choose).

## License

Licensed under either the [MIT license](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option.
