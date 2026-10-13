# PUT, PATCH, DELETE, and no-content responses

The HTTP binding now supports GET, POST, PUT, PATCH, and DELETE. The
[user example](../examples/contract/src/lib.rs) exercises each method with the
same contract, Axum service, and generated Rust/TypeScript clients.

## Binding rules

| Method | Operation kind | Request body |
| --- | --- | --- |
| GET | Query | None. |
| POST | Query or mutation | Optional explicit mapping. |
| PUT | Mutation | Required explicit JSON body mapping. |
| PATCH | Mutation | Required explicit JSON body mapping. |
| DELETE | Mutation | None in this profile. |

Remaining scalar arguments still map to path or query parameters. Success status
defaults to 200; 201 and 204 are explicit alternatives. No retry behavior is
inferred from the method. The application implements the operation's effects and
its idempotency semantics.

```rust
#[mutation]
#[http(put = "/users/{id}", body = "input")]
async fn replace(&self, id: UserId, input: ReplaceUser) -> Result<User, ApiError>;

#[mutation]
#[http(patch = "/users/{id}", body = "input")]
async fn update(&self, id: UserId, input: UserPatch) -> Result<User, ApiError>;

#[mutation]
#[http(delete = "/users/{id}", status = 204)]
async fn delete(&self, id: UserId) -> Result<(), ApiError>;
```

Both clients retain positional parameters. They construct the logical named
argument object internally, select `input` as the HTTP body, and encode `id` as
one path segment. Different verbs can share the same route pattern.

## Complete replacement versus partial update

The example's `ReplaceUser` describes the full writable profile. `display_name`
is required. Its nullable `bio` becomes `None` when omitted, so PUT replaces that
field rather than preserving its previous value. Repeating the same replacement
produces the same stored profile. This is application behavior demonstrated by the
example; the transport does not perform replacement automatically.

PATCH uses an explicit field-wise DTO:

```rust
use spline::{Patch, Schema};

#[derive(Default, serde::Serialize, serde::Deserialize, Schema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UserPatch {
    #[serde(default, skip_serializing_if = "spline::Patch::is_missing")]
    display_name: Patch<String>,

    #[serde(default, skip_serializing_if = "spline::Patch::is_missing")]
    bio: Patch<Option<String>>,
}
```

`Patch<T>` has two variants, `Missing` and `Value(T)`. Combining it with a nullable
payload gives the three states needed for an update:

| JSON | Rust `bio` value | Example effect |
| --- | --- | --- |
| `{}` | `Patch::Missing` | Leave the existing bio unchanged. |
| `{"bio": null}` | `Patch::Value(None)` | Clear the bio. |
| `{"bio": "Hello"}` | `Patch::Value(Some("Hello".into()))` | Replace the bio. |

`Patch<String>` does not admit null, so `{"displayName": null}` is rejected. An
empty patch is a no-op in the example, and unknown properties remain rejected.
Application validation runs before applying fields, so an invalid name does not
partially apply a valid bio change.

The annotations have separate jobs:

- `default` turns an absent property into `Patch::Missing`.
- `skip_serializing_if` omits that state from outgoing JSON.
- A present value serializes/deserializes directly as `T`.

Without the omission annotation, serializing `Patch::Missing` returns an encoding
error instead of silently turning it into null. Without a field default, omission
is a deserialization error, even for `Patch<Option<T>>`. A newtype visitor prevents
Serde's normal missing-Option behavior from collapsing absence into `Value(None)`.
The type's `Default` implementation does not require `T: Default`.

This is a typed application JSON patch DTO, not an implicit implementation of
JSON Merge Patch or JSON Patch. Nested objects/arrays are values of `T`; recursive
merging and update application remain explicit application decisions. The helper
is available from `spline` without enabling the HTTP feature.

## Client usage

```rust
client.update(id, UserPatch {
    bio: Patch::Value(None),
    ..UserPatch::default()
}).await?;
```

The generated TypeScript input is structurally:

```typescript
type UserPatchInput = {
  displayName?: string;
  bio?: string | null;
};

await client.update(id, {});                // unchanged
await client.update(id, { bio: null });     // clear
await client.update(id, { bio: "Hello" });  // replace
```

With `exactOptionalPropertyTypes`, explicit `undefined` is not interchangeable
with omission in these types. At runtime the client validates the JSON encoding,
where JSON serialization omits undefined object properties. Invalid nulls and
unknown fields are still rejected before fetch and by server decoding.

## 204 is distinct from JSON null

For macro-authored HTTP contracts, `status = 204` requires a Rust unit output
`Result<(), E>`. Type aliases of `()` work; nullable values, strings, and custom
null-encoded DTOs do not satisfy the sealed `NoContent` bound. Lower-level metadata
construction also checks that the output schema resolves directly to null.

On success the server emits an empty 204 response with no JSON content type or
error marker. It does not serialize `()` into the four bytes `null`. The response
helper also rejects direct misuse with a non-null success value rather than
silently discarding it.

| Declared success | Actual body | Rust client value | TypeScript client value/type |
| --- | --- | --- | --- |
| 200/201 with Rust unit | JSON `null` | `()` | `null` |
| 204 with Rust unit | Empty | `()` | `undefined`, typed as `void` |

Only the operation's binding-level output alias becomes `void`. Standalone schema
export and other operations still describe Rust unit's JSON representation as
`null`; DTO definitions are not rewritten globally.

Errors retain their ordinary application/framework JSON envelopes and error
statuses, even for an operation whose success status is 204. The example returns
a typed 404 when deleting an already missing user. Repeated DELETE requests leave
the same resource state, even though their status/result may differ.

Both clients enforce the declared success status. Switching between 200 JSON null
and 204 empty is a wire-breaking change, not an interchangeable encoding. For a
204, clients skip JSON parsing, reject error markers and forbidden Content-Length
or Transfer-Encoding headers, and require any body exposed by the HTTP transport
to be empty. A Content-Type header alone is not needed for decoding and is ignored.
Underlying HTTP implementations can discard illegal bytes after a 204; the
binding validates the framing and body those implementations expose.

## Verification

The workspace passes **104 behavior/integration tests** and **22 compiler-failure
fixtures** with the HTTP feature enabled. The TypeScript suites have **49
expected-error assertions**. Existing evolution fixtures continue to pass.

Evidence includes:

- Serde round trips preserving missing, null, and value, plus schema presence and
  nullability tests for the generic helper and a type alias.
- Generated Rust and TypeScript clients replacing, updating, clearing, and
  deleting records against the same live Axum server.
- Raw HTTP checks for exact status/body/headers and rejected malformed updates.
- Typed application errors after deletion and on bodyless-success operations.
- 200 JSON-null versus 204 empty behavior, including zero-argument operations.
- Native raw-server and fetch-double tests for unexpected statuses, forbidden
  framing, error markers, and observable bodies on 204.
- Compile-time rejection of query-only mutation verbs, missing PUT/PATCH body
  mappings, DELETE bodies, unsupported success statuses, and non-unit 204 results.

```sh
cargo test --locked --workspace
cargo test --locked -p spline --features http --test ui
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo check --locked -p spline --no-default-features
cargo run --locked -p spline-example-contract -- --check
npm run check
```

The normal workspace run enables HTTP through the example. The extra 204 compiler
fixture is feature-gated; the metadata-only crate remains independently usable.
