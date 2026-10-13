# Contract evolution and response compatibility

Implemented and checked: 2026-10-13.

## Policy

Generated HTTP clients now default to **compatible response decoding**:

- Validate known fields' types, required presence, nullability, and supported
  numeric constraints.
- Project closed response objects onto the properties declared by the client.
  Additional properties are discarded recursively before returning a value.
- Preserve map keys and validate their values. An additional dictionary entry is
  data, not an unknown DTO field.
- Preserve explicitly open objects according to their schema.
- Keep generated enums closed: unknown variants are protocol failures, including
  newly emitted application-error variants.

This applies to success bodies and application-error payloads. Request validation
continues to follow the authored input schema and Serde behavior. Receiver
tolerance does not rewrite schema definitions or widen generated static types.

## Choosing a policy

```rust
use spline::http::ResponsePolicy;

let client = users::Client::new(base_url)?; // compatible
let strict = users::Client::new(base_url)?
    .with_response_policy(ResponsePolicy::Strict);
```

```typescript
const client = createUsersClient(baseUrl); // compatible
const strict = createUsersClient(baseUrl, { responsePolicy: "strict" });
```

The Rust client checks/projects JSON using the known serialization schema before
deserializing the local output/error DTO. Thus `#[serde(deny_unknown_fields)]` can
still enforce a strict server request contract while compatible clients accept
added response fields: the extra properties are removed before Serde sees them.
Known required output fields remain required even if a deserializer could supply
a default for an absent value.

The TypeScript client returns the projected value rather than merely accepting a
wider value and casting it. Added properties do not silently leak into subsequent
code or re-serialized request objects. Explicitly declared maps/open objects
retain their extra entries.

Strict mode rejects extra properties on closed objects, including nested response
and error payload objects. It still respects explicitly open schemas; it does not
turn a map into a closed DTO.

## Evolution rules

The evaluated deployment direction is **old contract client → new server**, with
stable operation identity, routing, success status, and semantics of existing fields.

| Server change | Compatible old client | Strict old client |
| --- | --- | --- |
| Add an output-object property | Accepted; property is projected away. | Rejected for a closed object. |
| Add a nested property, including inside arrays/map values | Same recursive projection. | Rejected for a closed nested object. |
| Add a property to a known application's error payload | Existing typed error is retained; extra property is removed. | Rejected for a closed payload. |
| Add a dictionary entry with a valid value | Accepted and retained. | Accepted and retained. |
| Remove a required output property | Protocol failure. | Protocol failure. |
| Change a known type, emit disallowed null, or exceed an integer range | Protocol failure. | Protocol failure. |
| Add an output/error enum variant but keep emitting old variants | Existing variants remain decodable. | Existing variants remain decodable. |
| Emit that new variant to an old client | Protocol failure. | Protocol failure. |

Request-side rules are independent:

| Server input change | Old client → new server |
| --- | --- |
| Add a DTO field with a default or supported missing-field behavior | Compatible when old requests omit the new field. |
| Add an optional query argument accepting omission | Compatible when old requests omit it. |
| Add a required request field | Breaking: old calls receive an invalid-request framework error. |
| Expand an accepted input enum | Old values remain accepted; old clients still expose only their known variants. |
| Tighten type/range/validation/presence requirements | Potentially breaking; evaluate against old requests. |
| Rename wire properties/query keys/routes, or change the HTTP method/success status | Treat as wire-breaking unless the old mapping remains available. |

These are structural rules. Changing authorization, side effects, custom
predicates, or default values can change behavior even with compatible schemas.

### Wire compatibility and source compatibility differ

Adding an optional query argument can preserve old wire requests while changing a
newly generated positional method's arity. Adding a defaulted Rust DTO field can
also require edits to struct literals when upgrading the contract dependency.
Those source changes are separate from whether an existing client can continue
communicating with a server.

The reverse deployment direction is not implied. A new client sending a new field
to an old server may be rejected by its strict input contract. Deploying the
accepting server first, then sending new inputs, is the compatible order for the
tested additive-input changes.

### Enum additions need an explicit strategy

Generated output/error types remain discriminated and exhaustive. A new variant
is not silently coerced into an old variant, changed to null, or returned as an
untyped application error: it becomes `Protocol` / `kind: "protocol"`.

Known variants with added object fields can evolve through projection. If multiple
union alternatives match but produce different projected values, both clients
reject the ambiguity. Equivalent alternatives, such as repeated null branches in
nested Options, are allowed. An opt-in unknown-variant representation would need
an explicit public type and is separate future work.

## Versioned evidence

[The declarations](../examples/contract/fixtures/evolution.rs) define independent
`v1` and `v2` contracts with identical logical names/routes. V1 remains the older
schema. V2 adds response fields, nested fields, a defaulted input field, an optional
query argument, and output/error variants. A third negative fixture adds a required
input field.

[Rust tests](../examples/contract/tests/evolution.rs) run a generated v1 native
client against the actual v2 Axum router. They execute the
[generated v1 TypeScript client](../examples/contract/typescript/evolution-v1.generated.ts)
against those same servers using [Node assertions](../examples/contract/typescript/evolution.mjs).
Strict mode is also tested against the original v1 server.

Cases include nested/array/map-value additions, strict old DTOs, properties added
to adjacent/external enum payloads, new enum/error variants, missing or mistyped
known fields, invalid map values, nullability, numeric ranges, ambiguous enum
envelopes, input defaults, optional queries, and required-input failures. They
also confirm that request validation remains intact and a dictionary key named
`__proto__` is preserved as an own data property. Runtime unit tests cover
recursive projection, tuple arity, open objects, and traversal limits.

These fixtures use **the old API contract with the current Spline runtime**.
Clients generated/compiled before this receiver policy retain their previous
behavior until regenerated/rebuilt. The new v1 artifact establishes a baseline
for future compatibility work, not a retroactive promise about historical clients.

Custom schema overrides remain author-supplied descriptions. Rust custom
deserializers still run on projected values and may reject them. Open/opaque
schemas need their own conformance evidence when they depend on extra properties.

## Verification and regeneration

The expanded workspace now passes **104 behavior/integration tests** plus **22
compiler-failure fixtures** with HTTP enabled. The TypeScript suites include **49
expected-error assertions**; the original evolution cases remain covered.

```sh
cargo test --locked -p spline-example-contract --test evolution
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo run --locked -p spline-example-contract -- --check
npm run check
```

The existing contract generator checks/regenerates both the public user client and
the v1 evolution client. Use `--write` for intentional regeneration. Rust tests
check freshness, and TypeScript verifies that old types do not become wider merely
because response decoding tolerates additions.

## HTTP method coverage

The binding now supports **GET, POST, PUT, PATCH, and DELETE**, with JSON success
statuses **200/201** and empty **204** successes. The [update profile](http-updates.md)
defines explicit PATCH missing/null/value semantics and tests both clients through
the new operations. Switching an existing operation between 200 JSON null and 204
empty is a wire-breaking status/representation change, even though both correspond
to Rust unit at the implementation boundary.
