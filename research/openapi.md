# OpenAPI exporter and Utoipa evaluation

## Decision

Spline exports OpenAPI 3.1.2 from its sealed directional schema graph and HTTP
descriptions. `spline-openapi` uses an inspectable `serde_json::Value` document,
builder-style deployment configuration, and deterministic rendering. Its maintained
evidence lives in [the exporter tests](../crates/spline-openapi/tests) and
[independent validation helpers](../scripts/openapi-validation.mjs).

## Utoipa source review

Utoipa 6.0.0's document builders, reusable components, explicit deployment
modifiers, and separate UI integrations informed the design. Its schema/path derives
solve an authoring problem that Spline already addresses with compiled metadata.

Several source-level differences matter when exporting Spline's contract:

- `ResponsesBuilder::response` inserts into a status-keyed map; repeated statuses
  replace earlier responses. Spline explicitly groups application and framework
  alternatives before writing each status.
- `OpenApi::merge` has shallow name/path-based conflict behavior. Spline validates
  composition and component-name collisions before document construction.
- Utoipa's `IntoResponses` documents unit response variants as bodyless. Spline's
  unit application errors still have JSON envelopes; only a selected 204 success
  is bodyless.
- Spline's recursive registry and Deserialize/Serialize shapes already establish
  directional field presence and nullability. Those shapes remain authoritative.

## Executable compatibility probe

The evaluated candidate was `utoipa = { version = "=6.0.0", default-features = false }`
with Serde JSON 1.0.154 and Spline's pinned `nightly-2026-02-25` toolchain. The temporary
probe prepared recursive objects, tuple prefix items, literal constraints, u32 bounds,
overlapping null unions, and reference-local descriptions for a JSON round trip.

Compilation failed inside Utoipa with macros disabled. Its `openapi/schema.rs`
referenced `crate::utoipa::Number`, while that module was gated behind `macros`;
additional primitive `PartialSchema` implementations were unavailable. Thus the
candidate failed the selected dependency profile before serialization could be
evaluated. This is evidence about that exact version/configuration, not a claim
about all Utoipa versions or its default macro-enabled build.

Spline adopted the document-builder patterns and a JSON-backed output, with
independent standard validation. The temporary probe and its isolated manifest/
lockfile are outside the repository; no second schema derive or Utoipa dependency
is required by the exporter. A future object-model dependency can be reevaluated
against the maintained conformance cases.

## Validation and integration

Swagger Parser 13.1.0 supports OpenAPI 3.1 and its schema validator uses Ajv's 2020
engine. Ajv 8.20.0 is also pinned directly for instance validation. The suite resolves
all local references and validates emitted schemas against actual Serde samples and
live HTTP responses, including exact-status precedence over `4XX`/`5XX` ranges.

Utoipa Swagger UI 10.0.1 exposes an external-JSON document integration. This provides
a future UI path independently of the chosen document representation. Its external
document API does not itself perform standard validation.

## Sources

- [Utoipa 6.0.0](https://docs.rs/utoipa/6.0.0/utoipa/)
- [Document model and merge behavior](https://docs.rs/utoipa/6.0.0/utoipa/openapi/struct.OpenApi.html)
- [Response insertion source](https://github.com/juhaku/utoipa/blob/utoipa-6.0.0/utoipa/src/openapi/response.rs)
- [Feature/dependency source](https://github.com/juhaku/utoipa/blob/utoipa-6.0.0/utoipa/Cargo.toml)
- [IntoResponses semantics](https://docs.rs/utoipa/6.0.0/utoipa/derive.IntoResponses.html)
- [Swagger UI external JSON](https://docs.rs/utoipa-swagger-ui/10.0.1/utoipa_swagger_ui/struct.SwaggerUi.html#method.external_url_unchecked)
- [Swagger Parser 13.1.0 schema validator](https://github.com/APIDevTools/swagger-parser/blob/v13.1.0/lib/validators/schema.js)
- [OpenAPI 3.1.2](https://spec.openapis.org/oas/v3.1.2.html)
