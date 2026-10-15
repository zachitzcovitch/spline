# User contract: HTTP server and two clients

For a guided first application, start with the [blog tutorial](../blog/README.md).
This directory includes the CRUD example and deeper protocol conformance fixtures.

[src/lib.rs](src/lib.rs) defines get/create/replace/update/delete, Serde/schema DTOs,
and a shared in-memory Rust implementation of the generated server trait. The
explicit `module = "users"` declaration names the public companion namespace.

`users::describe()` describes the neutral contract; `users::http::describe()` selects
the HTTP binding for export tooling. The generator emits
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

Each client reads the seeded user, creates and fetches a record, replaces its
profile, partially updates it, and deletes it with a bodyless 204. The clients also
handle a typed NotFound error. Node 25.2.1 is the tested runtime for executing the
generated TypeScript directly.

`UserPatch` uses `spline::Patch<T>`: an omitted field stays unchanged, a supplied
value replaces it, and `bio: null` clears the nullable bio. A null display name is
invalid. PUT uses a complete replacement DTO, so its omitted bio becomes null.
See [updates and no-content responses](../../docs/http-updates.md) for the wire rules.

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

## OpenAPI fixture

[openapi.json](openapi.json) documents the CRUD/PATCH/204 profile from the selected
HTTP description. The workbench's executable error policy has opaque status
metadata, so error fallbacks retain the full application envelope without inventing
exact variant/status mappings. The [blog tutorial](../blog/README.md) demonstrates
declared defaults.

```sh
cargo run --locked -p spline-example-contract --example contract-openapi -- --write
cargo run --locked -p spline-example-contract --example contract-openapi -- --check
npm run check:openapi
```

The OpenAPI exporter is a development-only Cargo example. See the
[OpenAPI guide](../../docs/openapi.md) for schema and response rules.

## Evolution fixtures

The [versioned contracts](fixtures/evolution.rs) and
[evolution tests](tests/evolution.rs) exercise a v1 client against a v2 server.
The same generator command also maintains
[evolution-v1.generated.ts](typescript/evolution-v1.generated.ts), checked by
TypeScript and executed by the test suite.

```sh
cargo test --locked -p spline-example-contract --test evolution
```

Compatible response decoding is the default in both clients. The tests cover
strict opt-in behavior, additive fields, optional inputs, and breaking enum or
required-field changes. See [the evolution guide](../../docs/contract-evolution.md).

## Application composition fixtures

[fixtures/application.rs](fixtures/application.rs) composes `Users` and `Posts`
with `spline::api!`, a shared store, request-scoped authentication, and different
resource error types. The existing generator also maintains
[application.generated.ts](typescript/application.generated.ts).

```sh
cargo test --locked -p spline-example-contract --test application
```

The test starts an authenticated application server and executes grouped Rust and
TypeScript clients against it. See [application composition](../../docs/application-composition.md)
for the API and configuration, or the [blog walkthrough](../blog/README.md) for a
step-by-step application using those APIs.
