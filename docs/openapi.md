# OpenAPI export

`spline-openapi` generates deterministic **OpenAPI 3.1.2 JSON** from a selected
HTTP description. It consumes the same routes, directional schemas, and declared
error responses used by Spline's adapters.

## Export an application

Add `spline-openapi` as a development dependency of the contract crate and run the
exporter as a Cargo example, as in the [blog](../examples/blog/contracts/examples/export.rs):

```rust
use spline::http::DeclaredErrors;
use spline_example_blog_contracts::api;
use spline_openapi::OpenApi;

let binding = api::http::describe_with(&DeclaredErrors)?;
let document = OpenApi::new("Blog API", "0.1.0")
    .server("/api")
    .bearer_auth("bearerAuth")
    .export_application(&binding)?;

std::fs::write("openapi.json", document.to_pretty_json()?)?;
```

Use `export_contract(&resource::http::describe_with(&policy)?)` for one resource.
The returned `Document` supports `as_json()`, `as_json_mut()`, `into_json()`,
`to_json()`, and `to_pretty_json()`, and implements Serde `Serialize`. The rendering
helpers sort object keys; pretty JSON ends with a newline for checked-in artifacts.
Arrays retain their authored/configured order.

Title and version are explicit API-document metadata. The description defaults to
the resource/application documentation and can be set with `.description(...)`.
You can inspect or customize the document before writing it:

```rust
document.as_json_mut()["info"]["description"] = "Deployment-specific documentation".into();
```

Declare the variable `mut` for such edits, then validate the final document with
standard tooling. The exporter itself obtains its paths and schemas from sealed,
validated descriptions; arbitrary manual edits have their own semantics.

## Servers and authentication

`.server(...)` adds a static HTTP(S) or root-relative server URL, including any
mount prefix. The blog supplies `/api` because the application nests its router
there. Resource group names do not imply URL prefixes.

Server URLs in this profile have no credentials, query, fragment, template
variables, or whitespace. Multiple `.server(...)` calls retain their configured
order. With no server configured, OpenAPI's default server URL is `/`.

`.bearer_auth("bearerAuth")` both declares an HTTP bearer security scheme and
requires it globally. It does not label tokens as JWTs. Authentication remains
application behavior: an executable request factory does not provide an inferable
security scheme. Use explicit document configuration for the deployed API.

## Operations and request mapping

- `operationId` is the fully qualified logical operation ID, independent of a
  Rust method rename. Application resource groups become tags.
- Documentation carries through to operation summaries/descriptions, tags, schemas,
  fields, and parameters.
- Path parameters retain scalar constraints and are required. Query parameters
  use URL form encoding and their authored wire names.
- An optional nullable query argument is documented as optional with a non-null
  present-value schema. A null argument means omission, not a literal `null` query value.
- A selected JSON body is the argument value directly, without a generated named
  argument wrapper. The body is required even when its JSON value may be null.
- 200/201 successes carry the Serialize output schema. 204 has no content. A 200
  JSON-null success and a bodyless 204 remain distinct.

The canonical content type is `application/json`. Request-body prose also records
the binding's accepted `+json` media types and fixed byte limit. Unknown/duplicate
query keys and byte-level constraints are documented where OpenAPI schemas do not
express the full wire rule.

## Directional schemas and component names

The exporter uses JSON Schema 2020-12 semantics: explicit null types, `anyOf` for
unions, exact integer bounds, required-property lists, and open/closed object rules.
Maps retain their value schema. Fixed tuples use `prefixItems` with exact lengths.
Recursive and generic definitions retain references through components.

Public component names follow the same structural naming policy as TypeScript:

- One selected direction uses the plain name, such as `User`.
- Equivalent input/output shapes share one component.
- Differing shapes use names such as `UserInput` and `UserOutput`.
- Concrete generics use names such as `PageOfUser`.

Generated dispatch `.Args` wrappers are omitted. Source logical identities remain
independent of component presentation names. Collisions fail explicitly; use
`.name(DefinitionId, "ComponentName")` for directional overrides:

```rust
use spline::{DefinitionId, Direction};

let exporter = OpenApi::new("API", "1").name(
    DefinitionId { name: "billing.User".into(), direction: Direction::Serialize },
    "BillingUser",
);
```

Component names use OpenAPI's ASCII letters/digits/dot/underscore/hyphen grammar.
`ServiceError` is reserved for the request/service error envelope. Assign both equivalent
directions the same override to share that name; different shapes cannot share one
component. Structural equivalence is shared with TypeScript through the neutral
schema graph, including nested references and recursion.

Presence and nullability are distinct. An accepted omission does not invent a
concrete JSON default. Schema validation describes structural JSON values; arbitrary
custom codecs, lexical restrictions, and application business rules still need their
own conformance evidence. Native shared-DTO client compatibility is a separate check.

## Customer-facing error responses

Use `::http::describe_with(&DeclaredErrors)` or another declared policy to export
authoritative application statuses. The [error guide](http-errors.md) describes the
derive and custom policy mappings.

Application errors and framework faults can use the same HTTP status. The exporter
groups them under one Response Object with alternative envelope schemas. A request
factory can reject with 404 even when the resource also declares a 404 application error.

Framework faults use a reusable `ServiceError` component describing
`kind`, `code`, and `message`; the HTTP status is not a JSON body property. Common
decoding/size/encoding/service failures have explicit responses. `4XX` and `5XX`
framework-only fallbacks use the reusable `ServiceErrorResponse` response. Exact
response keys take precedence over ranges, so explicit error responses also include
the framework alternative.

No-policy descriptions and opaque executable policies retain their full application
error envelope in the fallback responses, without inventing exact variant/status
mappings.
The [CRUD artifact](../examples/contract/openapi.json) demonstrates an opaque policy;
the [blog artifact](../examples/blog/openapi.json) demonstrates derived defaults.

Generated documents use neutral component names and customer-facing descriptions.
Internal transport marker headers and exporter/policy extensions are omitted from
the public spec. Response bodies retain their serialized shapes and discriminator
values; application-authored names and documentation are exported as authored.
Live conformance tests also check marker/envelope agreement on actual responses.

## Artifacts and verification

From the repository root, regenerate both example documents:

```sh
cargo run --locked -p spline-example-blog-contracts --example blog-export -- --write
cargo run --locked -p spline-example-contract --example contract-openapi -- --write
```

The blog command writes both TypeScript and OpenAPI. The workbench keeps its
OpenAPI generator in a separate Cargo example so the exporter is development-only.
Check freshness and validate with independent tools:

```sh
cargo run --locked -p spline-example-blog-contracts --example blog-export -- --check
cargo run --locked -p spline-example-contract --example contract-openapi -- --check
npm run check:openapi
cargo test --locked -p spline-openapi
npm run test:blog
npm run check:neutral
```

Install root npm dependencies before the Rust conformance suite. Swagger Parser
13.1.0 validates the documents and references; Ajv 8.20.0 independently validates
real Serde samples, invalid instances, and live response bodies. Tests cover input/
output differences, numeric bounds, recursion, tuples, PATCH, required-nullable
bodies, optional queries, shared error statuses, opaque policies, and empty 204.

The exporter depends on lightweight metadata and JSON/URL tooling. The dependency
check guards both the facade's neutral execution graph and the exporter's separation
from Axum/reqwest/Tower execution. See [the Utoipa evaluation](../research/openapi.md)
for the document-model decision. A standard JSON document can be consumed by
documentation UI tools as a separate integration.
