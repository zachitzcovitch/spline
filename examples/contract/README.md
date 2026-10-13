# User contract: HTTP server and two clients

[src/lib.rs](src/lib.rs) defines one query and one mutation, Serde/schema DTOs,
and a shared in-memory Rust implementation of the generated server trait. The
explicit `module = "users"` declaration names the public companion namespace.

`users::describe()` builds the metadata for export tooling. The generator emits
[typescript/generated.ts](typescript/generated.ts), including wire types,
`UsersOperations`, positional `UsersCalls` signatures, and `createUsersClient`.

Start the server in one terminal:

```sh
cargo run --locked -p spline-example-contract --bin server -- 127.0.0.1:3000
```

Then run the clients:

```sh
cargo run --locked -p spline-example-contract --bin rust-client -- http://127.0.0.1:3000
node examples/contract/typescript/client.mjs http://127.0.0.1:3000
```

Each client reads the seeded user, creates a user, fetches the new record, and
handles a typed NotFound error. Node 25.2.1 is the tested runtime for executing
the generated TypeScript directly.

From the repository root:

```sh
cargo run --locked -p spline-example-contract -- --write
cargo run --locked -p spline-example-contract -- --check
cargo test --locked -p spline-example-contract
npm run check --workspace spline-contract-typescript-checks
```

The HTTP suite starts ephemeral loopback servers and executes both clients, plus
request-context, URL encoding, and error-path checks. See the
[HTTP binding guide](../../docs/http-binding.md) for the profile and client errors.
