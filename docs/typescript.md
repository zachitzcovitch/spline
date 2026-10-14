# TypeScript clients and public types

Spline exports ordinary DTO names and signatures intended for direct imports and
editor use. For the blog application:

```typescript
import { createBlogClient, type NewPost, type Post, type UserError } from "./generated.ts";

const client = createBlogClient("http://localhost:3000/api", {
  headers: { authorization: "Bearer alice-token" },
});
const input: NewPost = { title: "Hello", body: "Text" };
const result = await client.posts.create(input);
if (result.ok) {
  const post: Post = result.value;
  console.log(post.id);
}
```

Signature help shows the authored argument name and public DTOs:

```typescript
create(input: NewPost): Promise<BlogResult<Post, PostError, BlogHttpFailure>>
get(id: string): Promise<BlogResult<User, UserError, BlogHttpFailure>>
```

After narrowing `result.ok`, a successful user result is `User`; after narrowing
the application failure category, its declared error is `UserError`. Property
completion follows those concrete types.

## Default names

`#[spline(name = "blog.UserError")]` assigns a logical identity used by the schema
graph and runtime. The TypeScript name comes from its final component: `UserError`.
Inferred Rust crate/module-qualified identities use the same final-component rule.

| Exported schema | Public TypeScript name |
| --- | --- |
| Only one direction of `blog.User` is used | `User` |
| Both directions of `blog.User` have equivalent shapes | One shared `User` |
| Both directions have differing shapes | `UserInput` and `UserOutput` |
| Concrete generic `blog.Page<blog.User>` with equivalent directions | `PageOfUser` |

Input is the server's **Deserialize** view; output is its **Serialize** view.
Equivalence follows references, including generic and recursive types. Field
names, presence, nullability, numeric ranges, enum alternatives, and unknown-field
rules must agree. Differences in nested DTOs keep their enclosing types directional
too. Documentation is not part of the shape comparison.

Contract/application export selects the DTOs used by public inputs, outputs, and
errors. It omits generated dispatch argument wrappers. The full directional graph
and schema validation rules remain available to the runtime. This permits clean
names without weakening request validation or response projection.

Names are deterministic for the same exported graph. Introducing an opposite
direction with a differing shape can change `User` to `UserInput`/`UserOutput`;
that is an intentional distinction. Adding a type with the same short name produces
a collision error rather than assigning visit-order-dependent suffixes.

## Collisions and explicit names

If `billing.User` and `accounts.User` are exported together, both would default to
`User`. Give one or both a deliberate name in the Rust exporter:

```rust
use spline::{DefinitionId, Direction};
use spline_typescript::Typescript;

let exporter = Typescript::new().name(
    DefinitionId {
        name: "billing.User".into(),
        direction: Direction::Serialize,
    },
    "BillingUser",
);
let artifact = exporter.export_http_application(&api::http::describe()?, "Api")?;
```

Overrides are direction-specific and update references as well as declarations.
Assign the same override to both equivalent directions to share a custom name,
or different overrides to expose them separately. An override on just one direction
opts out of automatic sharing for that type. Differing shapes cannot share one
declaration even through explicit overrides.

Reserved TypeScript symbols and names colliding with generated utilities are
rejected. Concrete generic names may also collide; use explicit overrides rather
than relying on the generated spelling as a globally unique identity.

## Results and metadata

Exporting an HTTP application named `Blog` supplies:

- Public DTOs such as `User`, `Post`, `NewPost`, `UserError`, and `PostError`.
- `BlogClient`, returned by `createBlogClient`.
- `BlogCalls<Failure = never>`, the callable surface with an optional extra failure
  type for bindings.
- `BlogResult<Value, ApplicationError, Failure = never>`, the discriminated result.
- `BlogHttpOptions` and `BlogHttpFailure`, the common HTTP configuration/failures.
- `BlogOperations`, keyed by stable logical operation IDs, with input/output/error
  types available to tooling.

The operation map keeps named logical inputs, while callable signatures preserve
positional arguments. Tooling can index `BlogOperations["posts.create"]["output"]`
when needed; normal application code can use `Post` directly. Generated per-operation
input/output/error aliases are unnecessary and are no longer exported.

Normal parameter names are preserved, such as `id` and `input`. Reserved names or
names shadowing the runtime call helper receive a local `argN` fallback. The
original logical field name and wire mapping stay intact.

HTTP-bound 204 results use `void` in operation metadata and signatures; 200/201 unit
results use JSON `null`. Neutral contract/application export and standalone schema
export continue to represent unit's JSON shape as `null`.

## Migrating older generated imports

Regenerate from Rust and update imports, for example:

```typescript
// Before: blog_UserError_Output, blog_posts_create_Output, arg0_input
// Now:
import type { UserError, Post, BlogClient } from "./generated.ts";
```

`UserInput`/`UserOutput` remain appropriate when directions differ. Operation IDs,
HTTP routes, JSON payloads, positional order, and failure semantics retain their
authored meaning. These changes affect the generated TypeScript type surface.

## Editor verification

The [language-service test](../examples/blog/tests/typescript-editor.mjs) checks
the actual signature-help text, narrowed hover types, and DTO property completions
using the pinned TypeScript version. It runs in `npm run test:blog` alongside the
live cross-language workflow. Strict compiler assertions cover public imports,
directional shapes, result narrowing, and invalid calls.
