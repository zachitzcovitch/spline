# Spline roadmap

Spline is experimental and developed in this workspace. Public APIs and generated
artifacts may change between commits; packages are not yet published. The
[changelog](CHANGELOG.md) records implemented milestones. This roadmap describes
priorities rather than release-date commitments.

## Working today

- Rust-authored resource contracts, a Spline-owned directional schema graph, and
  Serde-backed derives.
- Axum HTTP/JSON dispatch and executable, positional Rust and TypeScript clients.
- GET/POST/PUT/PATCH/DELETE, explicit partial-update fields, and bodyless 204.
- Compatible/strict response decoding with cross-version conformance checks.
- Multi-resource applications, shared request context, and grouped clients.
- A [multi-crate blog tutorial](examples/blog/README.md) and cross-language CI.
- [Local performance benchmarks](benchmarks/README.md) for metadata, export, and clients.
- [Typed Tower execution and explicit bindings](docs/execution-bindings.md), with
  neutral composition and an in-process adapter alongside HTTP.
- Neutral service bundles with adapter-owned extension traits for protocol selection
  and request-scoped HTTP factories.
- [Declared HTTP error statuses](docs/http-errors.md), with named constants on error
  derives, policy-aware response metadata, and custom policy overrides.
- [OpenAPI 3.1.2 export](docs/openapi.md), with inspectable documents, directional
  components, application/framework responses, and independently validated artifacts.

## Next priorities

1. **First published release.** Review public APIs and documentation, verify
   package contents/dependency versions, establish a versioning policy, and publish
   a tested set of crates and generated-client guidance.

## Areas to investigate

- Request cancellation, per-call headers/timeouts, and other client call controls.
- Broader response metadata and additional HTTP conventions.
- Documentation UI integrations and richer OpenAPI deployment/security configuration.
- Compile-time and runtime cost for larger contracts; use measured benchmarks to
  guide optimizations while retaining schema and wire conformance.
- Additional transports, streaming, and broader Serde representations as explicit
  extensions to the supported profile.

See [DESIGN.md](DESIGN.md) for the broader direction and [research/](research/README.md)
for the original rationale. Suggestions and concrete use cases are welcome in
[issues](https://github.com/zachitzcovitch/spline/issues).
