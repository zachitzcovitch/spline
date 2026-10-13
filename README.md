# Spline

A Rust-first contract system for servers and clients. The experimental workspace
now connects Rust contracts to directional schema metadata, an Axum HTTP binding,
and executable Rust and TypeScript clients.

## Define a resource contract

From the [working example](examples/contract/src/lib.rs):

```rust
#[spline::contract(namespace = "users", module = "users")]
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
- Typed argument structs in the explicitly named `users` companion module.
- `users::router(service, error_status)` and `users::Client::new(base_url)`.
- `users::describe()` for export tooling and introspection. Server/client setup
  collects the metadata internally.

Export the metadata:

```rust
let contract = users::describe()?;
let artifact = spline_typescript::Typescript::new()
    .export_http_client(&contract, "Users")?;
```

The [generated TypeScript](examples/contract/typescript/generated.ts) includes
`createUsersClient`, typed results, and structural JSON validation. Both clients
preserve authored positional parameters:

```rust
let client = users::Client::new("http://localhost:3000")?;
let user = client.get(UserId("user_1".into())).await?;
```

```typescript
const client = createUsersClient("http://localhost:3000");
const user = await client.get("user_1");
const created = await client.create({ displayName: "New user" });
```

Enable the `spline` crate's `http` feature for bound contracts. The first binding
supports GET/POST, scalar path/query parameters, selected JSON bodies, and 200/201
success responses. See the [HTTP guide](docs/http-binding.md) for the wire format,
request-context factory, error policies, and supported DTO compatibility.

## Workspace

| Crate | Responsibility |
| --- | --- |
| `spline` | User-facing facade, macros, and generated-code integration. |
| `spline-schema` | Schema trait, directional graph, and registry. |
| `spline-contract` | Operation metadata and transactional contract construction. |
| `spline-macros` | Schema derive and contract attribute; private Serde adapter. |
| `spline-typescript` | Deterministic wire types and operation signatures. |
| `spline-http` | Axum dispatch support, HTTP errors, and the reqwest client runtime. |

`examples/contract` demonstrates one query and one mutation over a shared in-memory
service. Packages are currently unpublished.

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
cargo run --locked -p spline-example-contract --bin server -- 127.0.0.1:3000
```

Then run either client:

```sh
cargo run --locked -p spline-example-contract --bin rust-client -- http://127.0.0.1:3000
node examples/contract/typescript/client.mjs http://127.0.0.1:3000
```

The Node runner imports the generated TypeScript directly using built-in type
stripping; Node 25.2.1 is the tested runtime.

## Run and verify

From the repository root:

```sh
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo check --locked -p spline --no-default-features
cargo run --locked -p spline-typescript --example export -- --check
cargo run --locked -p spline-example-contract -- --check
npm ci --ignore-scripts --no-audit --no-fund
npm run check
```

Cargo and npm dependencies are locked at the workspace root. TypeScript 5.9.3 is
pinned, with strict checks, exact optional properties, and checked index access.
The HTTP integration suite starts loopback servers and launches the Rust and Node
clients, so a Node runtime with built-in TypeScript stripping is needed for the
complete workspace test run.

## Design and research

- [Workspace and contract milestone](docs/workspace-contracts.md): implemented API,
  crate boundaries, verification, and current scope.
- [First HTTP binding](docs/http-binding.md): authoring, server/client examples,
  wire behavior, request context, and conformance checks.
- [Overall design](DESIGN.md): agreed direction and the broader roadmap.
- [Research index](research/README.md): the experiments and source reviews behind
  the implementation, including where their code and tests now live.
