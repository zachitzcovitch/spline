# A small blog built with Spline

Start here to learn how to use Spline. This small monorepo connects **one shared
Rust contract crate** to a server crate, a Rust client crate, and a TypeScript package.

The walkthrough reads Alice's profile, creates and fetches a draft, handles Bob's
typed `Forbidden` error when he tries to publish it, then publishes as Alice.

## Project layout

```text
examples/blog/
├── contracts/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs          # Public exports and api! composition
│   │   ├── user.rs         # User DTO, error, and Users trait
│   │   └── post.rs         # Post DTOs, errors, and Posts trait
│   └── examples/export.rs  # TypeScript generation
├── server/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # Session factory and HTTP startup
│       ├── auth.rs         # Request authentication
│       ├── errors.rs       # Application error statuses
│       └── service/
│           ├── mod.rs      # Shared Store and request-scoped Session
│           ├── users.rs    # impl Users for Session
│           └── posts.rs    # impl Posts for Session
├── rust-client/
│   ├── Cargo.toml
│   └── src/main.rs         # CLI entry point, workflow, and client setup
├── typescript/
│   ├── run.mjs             # CLI arguments and startup
│   ├── client.ts           # Workflow, client setup, and result helper
│   └── generated.ts        # Generated from the contracts crate
├── tests/workflow.mjs      # Builds and exercises the actual applications
└── README.md
```

Both Rust applications declare a path dependency on `../contracts`. The contract
crate contains the DTOs, traits, and generated HTTP entry points:

```text
server ──────→ contracts ←────── rust-client
                   │
             export example
                   │
                   ▼
          generated TypeScript → TypeScript client
```

The three crates are members of the repository's **root Cargo workspace**, sharing
its `Cargo.toml` and `Cargo.lock`. The TypeScript package belongs to the root npm
workspace and shares `package-lock.json`. The exporter uses `spline-typescript`
as a development dependency of the contracts crate; application builds depend on
the contracts' normal dependencies (`spline` with HTTP support and Serde).

## Run the application

Run every command below from the **repository root**. This example uses the local
workspace crates; their packages are currently unpublished.

Install Rust through rustup and use the Node version pinned in `.nvmrc` (25.2.1).
With nvm installed, prepare the checkout:

```sh
rustup show
nvm install
nvm use
npm ci --ignore-scripts --no-audit --no-fund
```

The generated TypeScript client is checked in, so the first run is ready to go.

**Terminal 1 — start the server:**

```sh
cargo run --locked -p spline-example-blog-server
```

Wait for:

```text
Blog API: http://127.0.0.1:3000/api
```

**Terminal 2 — run the Rust client, then the TypeScript client:**

```sh
cargo run --locked -p spline-example-blog-client
node examples/blog/typescript/run.mjs
```

The first client prints:

```text
User: Alice
Created draft: post_1
Read draft: Hello from Spline
Bob cannot publish Alice's post (typed Forbidden)
Published: post_1 (published=true)
```

The second follows the same workflow with `post_2`. Each run creates another post.
The in-memory store is shared by both resources and clients; stopping the server
with Ctrl-C and restarting it resets the data.

The example seeds Alice and Bob and recognizes two fixed **local demo tokens**:
`alice-token` and `bob-token`. Both clients configure them in the Authorization
header. All routes require authentication; authenticated users may read drafts,
and only a post's author may publish it.

### Use another port

Supply the server's bind address and the clients' base URL, including `/api`:

```sh
cargo run --locked -p spline-example-blog-server -- 127.0.0.1:3001
cargo run --locked -p spline-example-blog-client -- http://127.0.0.1:3001/api
node examples/blog/typescript/run.mjs http://127.0.0.1:3001/api
```

Run the server in its own terminal, as above. An omitted `/api` prefix gives an
unmatched route; an unavailable server gives a transport failure.

## Read the code in this order

| File | What to look for |
| --- | --- |
| [contracts/src/lib.rs](contracts/src/lib.rs) | Public exports and the composed `api` module. |
| [contracts/src/user.rs](contracts/src/user.rs) | The user DTO, error, and `Users` trait. |
| [contracts/src/post.rs](contracts/src/post.rs) | Post DTOs, errors, and the `Posts` trait. |
| [server/src/main.rs](server/src/main.rs) | Shared store, request-scoped session factory, and Axum startup. |
| [server/src/auth.rs](server/src/auth.rs) | Turning a request header into the session's user ID. |
| [server/src/service/mod.rs](server/src/service/mod.rs) | Shared storage and the request-scoped `Session`. |
| [server/src/service/users.rs](server/src/service/users.rs) | The user resource implementation. |
| [server/src/service/posts.rs](server/src/service/posts.rs) | The post resource implementation. |
| [server/src/errors.rs](server/src/errors.rs) | Mapping each declared error to an HTTP status. |
| [rust-client/src/main.rs](rust-client/src/main.rs) | CLI entry point, generated Rust calls, typed errors, and client configuration. |
| [typescript/client.ts](typescript/client.ts) | The matching workflow, client configuration, and result helper. |
| [contracts/examples/export.rs](contracts/examples/export.rs) | Generating a TypeScript client from the shared contract. |

Server implementation lives in the server crate. Both Rust applications import
`spline_example_blog_contracts`, and TypeScript imports
[`generated.ts`](typescript/generated.ts). A four-line Node launcher,
[`run.mjs`](typescript/run.mjs), supplies the base URL to the type-checked client.
Node runs the `.ts` files directly using built-in type stripping.

## 1. Define the contract

Each resource file groups its DTOs, errors, and operations. `lib.rs` re-exports
those public types and the generated `users`/`posts` companion modules, then
composes them into `api`. For a focused read, follow
[`post.rs`](contracts/src/post.rs) →
[`service/posts.rs`](server/src/service/posts.rs) → a client call.

DTOs derive Serde's `Serialize`/`Deserialize` and Spline's `Schema`. Serde attributes
describe the wire names: `User.display_name` becomes `displayName` in JSON and
TypeScript. `#[spline(name = "blog.User")]` assigns a stable schema identity.

Each resource is an annotated Rust trait. For example, the `Posts` contract declares:

```rust
#[mutation]
#[http(post = "/posts", body = "input", status = 201)]
async fn create(&self, input: NewPost) -> Result<Post, PostError>;
```

This records the operation's types, mutation kind, HTTP path, body mapping, and
success status. Calling `create(new_post)` sends the `NewPost` JSON directly as
the request body. The authenticated author comes from the session.

The complete API has four operations:

| Identity | HTTP route (under `/api`) | Meaning |
| --- | --- | --- |
| `blog.users.get` | `GET /users/{id}` | Read a profile. |
| `blog.posts.create` | `POST /posts` | Create a draft; returns 201. |
| `blog.posts.get` | `GET /posts/{id}` | Read a post. |
| `blog.posts.publish` | `POST /posts/{id}/publish` | Publish your post. |

`publish` is a custom operation using the same contract machinery as `get` and
`create`. Publishing an already published post succeeds for its author.

Finally, compose the generated resource modules:

```rust
spline::api! {
    pub mod api {
        users: crate::users,
        posts: crate::posts,
    }
}
```

This generates the grouped Rust client, router constructors, and an application
description for export tooling. Resource names organize client accessors;
namespaces such as `blog.posts` preserve the operation identities.

## 2. Implement the server

`Session` implements `Users` and `Posts` with normal `async fn` methods. It holds
two things: an `Arc<RwLock<Store>>` shared across requests and the authenticated
user ID for the current request.

The shared types live in `service/mod.rs`; `service/users.rs` and
`service/posts.rs` implement the corresponding contract traits on that session.

The server wires it up once:

```rust
let routes = api::router_with(
    move |context| {
        let store = Arc::clone(&store);
        async move {
            let user_id = auth::authenticate(&context)?;
            Ok(Arc::new(Session { store, user_id }))
        }
    },
    errors::ErrorStatuses,
)?;
```

The factory runs before argument decoding and invocation for each matched
operation. `auth::authenticate` returns a user ID or a framework `Fault` with
status 401. Application authentication can use this same boundary to verify a
session or obtain a principal from middleware extensions.

`ErrorStatuses` implements `ErrorPolicy<UserError>` and `ErrorPolicy<PostError>`:

| Failure | HTTP status | Client category |
| --- | --- | --- |
| Missing/invalid demo token | 401 | Framework error (`unauthorized`). |
| User or post not found | 404 | Declared application error (`NotFound`). |
| Another user tries to publish | 403 | Declared application error (`Forbidden`). |
| Blank post title | 422 | Declared application error (`InvalidTitle`). |

The schema checks that a title is a string. The server applies the business rule
that its trimmed content must be nonempty. Mutations hold the write lock while
checking ownership and updating state.

`api::router_with` returns an Axum router. `main.rs` nests it under `/api`, which
is why that prefix belongs in both clients' base URLs.

## 3. Call it from either language

Each client keeps its workflow and small helpers together in one file:
[`rust-client/src/main.rs`](rust-client/src/main.rs) and
[`typescript/client.ts`](typescript/client.ts). Section comments separate the
workflow, client configuration, and error handling. The `run` functions read top
to bottom as the same sequence of generated API calls, with configuration helpers
below them. Rust's `main` and TypeScript's `run.mjs` supply the base URL and start
the workflow.

The two clients preserve the same authored positional arguments:

```rust
let user = alice.users().get("alice".into()).await?;
let new_post = NewPost {
    title: "Hello from Spline".into(),
    body: "One Rust contract, two typed clients.".into(),
};
let draft = alice.posts().create(new_post).await?;
```

```typescript
const user = valueOrThrow(await alice.users.get("alice"), "Get user");
const newPost = {
  title: "Hello from Spline",
  body: "One Rust contract, two typed clients.",
};
const draft = valueOrThrow(await alice.posts.create(newPost), "Create draft");
```

Rust returns `Result<Post, CallError<PostError>>`. The workflow uses `?` to propagate
unexpected failures and matches `CallError::Application(PostError::Forbidden)` to
handle the expected error from Bob's publish attempt.

TypeScript returns a discriminated result. The example-local `valueOrThrow` helper
extracts the inferred success value or throws a CLI error containing the operation
name and failure details. This keeps routine success handling concise.

For the expected `Forbidden` error, `client.ts` inspects the generated result
directly: check `result.ok`, then `result.error.kind === "application"`, then the
specific variant in `result.error.value.kind`. Each check narrows the generated
types, making the expected-error handling explicit in the workflow.

Authorization headers are configured once per client and apply to both groups.
Spline builds request URLs, encodes arguments, validates/decodes responses, and
preserves each operation's error type.

You can also see the plain HTTP representation:

```sh
curl -i -H "Authorization: Bearer alice-token" http://127.0.0.1:3000/api/users/alice
```

The success body is `{"id":"alice","displayName":"Alice"}`. Omitting the header
returns a structured framework error with status 401.

## 4. Change the contract and regenerate

After changing DTOs or operations in `contracts/src/user.rs` or
`contracts/src/post.rs`, update the corresponding implementation under
`server/src/service/` and regenerate the TypeScript artifact:

```sh
cargo run --locked -p spline-example-blog-contracts --example blog-export -- --write
npm run check --workspace spline-blog-client
npm run test:blog
```

The exporter calls `api::describe()` and
`Typescript::export_http_application(..., "Blog")`, producing `createBlogClient`,
directional wire types, and the `BlogOperations`/`BlogCalls` type maps. Edit the
Rust contract to change that generated file. Ordinary server/client startup
collects its metadata internally.

The Cargo example target is named `blog-export` to give it a unique executable
name within the shared workspace; its source is `contracts/examples/export.rs`.

Check freshness without rewriting the artifact:

```sh
cargo run --locked -p spline-example-blog-contracts --example blog-export -- --check
```

`npm run test:blog` runs [tests/workflow.mjs](tests/workflow.mjs). It builds the
server and Rust client packages with `cargo build --locked`, locates their binaries
from Cargo's JSON output, starts the server on an available loopback port, and runs
both README clients. It checks persisted posts, authentication, and title validation
through HTTP, then shuts down the server. The command works with a custom
`CARGO_TARGET_DIR` and handles building the applications on a fresh checkout.

This cross-package workflow has its own CI step alongside `cargo test --workspace`.
CI also checks the generated artifact and type-checks both the generated and
handwritten TypeScript.

For more detail, see [application composition](../../docs/application-composition.md),
[HTTP behavior](../../docs/http-binding.md), and
[response compatibility](../../docs/contract-evolution.md).
