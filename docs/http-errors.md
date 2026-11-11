# Declared HTTP application errors

An API error type can declare its default HTTP statuses with the opt-in
`splinekit::http::Error` derive. Enable the facade's `http` feature and use named
`StatusCode` constants:

```rust
use splinekit::http::StatusCode;

#[derive(Debug, serde::Serialize, serde::Deserialize, splinekit::Schema, splinekit::http::Error)]
#[serde(tag = "code", rename_all = "snake_case", deny_unknown_fields)]
pub enum PostError {
    #[http(status = StatusCode::NOT_FOUND)]
    NotFound,

    #[http(status = StatusCode::FORBIDDEN)]
    Forbidden,

    #[http(status = StatusCode::UNPROCESSABLE_ENTITY)]
    InvalidTitle,
}
```

Every variant needs a status. The attribute requires a constant `StatusCode`
expression; numeric literals and non-error statuses are compiler errors. Two
variants may share a status. Payloads use the same annotation:

```rust
#[http(status = StatusCode::UNPROCESSABLE_ENTITY)]
Invalid { field: String },
```

On the wire each error is problem details: the value tagged by `code`, plus the
HTTP `status`. `PostError::Invalid { field: "title".into() }` is sent as
`{"status":422,"code":"invalid","field":"title"}` with
`Content-Type: application/problem+json`. Give errors data, not prose: a client
branches on `code` and decides what its users read. Binding refuses error types
that aren't tagged by `code`, that declare one of the binding's own fault codes,
or that have a field named like an RFC 9457 member; see the
[wire profile](http-binding.md#wire-responses). Adjacent tagging
(`#[serde(tag = "code", content = "details")]`) works too and keeps payloads under
one field.

The derive implements `splinekit::http::Error`, supplying both an exhaustive runtime
match and its static mapping. It uses the type's `Schema` for the actual serialized
representation. Serde remains responsible for encoding.

## Select the declared defaults

`DeclaredErrors` is a reusable policy selecting each error type's declared defaults:

```rust
use splinekit::http::{DeclaredErrors, HttpExt, HttpFactoryExt};

let router = users::shared(implementation).http(DeclaredErrors)?;
let routes = api::contract().http_with(session_factory, DeclaredErrors)?;
```

It implements `ErrorPolicy<E>` for every `E: splinekit::http::Error`, so one policy
works across resources with distinct error types. Lower-level router constructors
and configured operation routers accept it too.

The [blog tutorial](../examples/blog/README.md) declares statuses directly on
`UserError` and `PostError`. Its server and export example select `DeclaredErrors`;
there is no separate table of executable status matches to maintain.

## Describe the selected policy

Descriptions with policy metadata are explicit:

```rust
use splinekit::http::DeclaredErrors;

let resource = users::http::describe_with(&DeclaredErrors)?;
let application = api::http::describe_with(&DeclaredErrors)?;
let errors = &application.plans()["posts.create"].errors;
```

Each plan contains `ErrorResponses`:

- `Declared(BTreeMap<u16, ErrorResponse>)`: validated application responses grouped
  by HTTP status.
- `Opaque`: no static status mapping is claimed. This is the result for no-policy
  `::http::describe()` and custom policies without a declaration.

An `ErrorResponse` has the declared errors at that status (`error`, as a client
decodes them), the complete problem body (`body`: those errors with `status`
added) and the content type, `application/problem+json`. For a 404 above, its body
describes:

```json
{"status":404,"code":"not_found"}
```

Several variants sharing a status produce a union. Shapes retain references into
the neutral description's Serialize graph, including generic and recursive
payloads. Serde variant/field renames and internal/adjacent tagging are preserved.

Routers use the same policy-aware validation path before installing routes.
Invalid declarations fail construction, including missing, duplicate, or unknown
variant rules supplied manually. Client setup can continue using the no-policy
description: typed error decoding then matches a declared `code` at any status,
independently of a particular server policy.

These are **application-error responses**. Framework faults, request-factory
rejection, service failures, and unrecognized HTTP errors have distinct meanings. In
particular, a request factory can return other 4xx/5xx framework faults; the table
does not claim to enumerate every possible HTTP response. See the
[HTTP wire profile](http-binding.md#wire-responses).

## Whole-error statuses and supported representations

An error type whose complete shape uses one status can declare a container
status:

```rust
#[derive(Debug, serde::Serialize, serde::Deserialize, splinekit::Schema, splinekit::http::Error)]
#[serde(tag = "code", rename_all = "snake_case")]
#[http(status = StatusCode::CONFLICT)]
pub enum Conflict {
    VersionChanged { current: u32 },
}
```

Choose either a container status or per-variant statuses. Per-variant declarations
support the existing derived tagged-enum profile, type-parameter generics, and type
aliases. `Schema` records source variant identities separately from wire names and
associates them with the directional union alternatives. This neutral metadata has
no HTTP statuses or dependencies.

Whole-type schema overrides and handwritten schemas do not automatically establish
source-variant associations. Use a container status for those representations, or
a custom policy. Explicit handwritten variant metadata is an author-supplied
schema declaration, just like a manual `Schema` implementation.

Skipped serialized variants are unsupported by per-variant status derives. Empty
error enums are outside the current schema profile. Tuple variants, untagged
enums, custom codecs, and other representations retain the existing schema support
boundaries; a whole-error status does not establish codec conformance.

## Custom policies and overrides

Use a custom `ErrorPolicy<E>` or closure when a reusable domain error needs an
application-specific HTTP interpretation. It can override a derived default:

```rust
let router = users::shared(implementation).http(|_: &UserError| StatusCode::CONFLICT)?;
```

Custom policies remain opaque by default. A policy with an authoritative static
mapping can also implement `ErrorPolicy::mapping()` and return:

- `ErrorMapping::Status(status)` for the complete error shape.
- `ErrorMapping::Variants(rules)` for an exhaustive source-variant/status mapping.

These lightweight metadata values use numeric statuses and are validated at binding
construction. Their declarations must cover all application values/statuses the
policy can emit for that configuration. Keep genuinely dynamic policies opaque
when they cannot provide an exhaustive description.

For manually constructed bindings, `description.errors(operation_id, mapping)`
attaches validated response metadata transactionally. The neutral description and
its schema graph remain available through `.contract()` or `.application()`.

## Verification

The [derive/binding suite](../crates/splinekit/tests/http_errors.rs) compares actual HTTP
responses against per-status metadata and native typed decoding. It covers payloads,
renames, shared statuses, generic/recursive errors, aliases, custom policies, and
transactional validation. Compiler fixtures verify status types, status ranges,
missing/conflicting declarations, skipped variants, and unsupported overrides.

```sh
cargo test --locked -p splinekit --features http --test http_errors --test ui
npm run test:blog
```

The blog workflow runs both real clients and checks 403/404/422 statuses and typed
problem bodies. Generated TypeScript still uses the same public error DTOs and
discriminated results. [OpenAPI export](openapi.md) uses this metadata to document
per-status application errors alongside possible framework faults.
