# Changelog

User-visible changes to Spline are recorded here. The project has no published
release yet; the workspace version is a development version.

## Unreleased

### Added

- Directional JSON schemas, recursive/generic registration, and Serde-aware derives.
- Resource-grouped Rust contracts with stable operation identities and positional
  Rust/TypeScript call signatures.
- Axum HTTP routing, request-scoped service factories, reqwest clients, and
  executable TypeScript fetch clients with typed failures.
- PUT/PATCH/DELETE, `Patch<T>` omission semantics, and bodyless 204 responses.
- Compatible/strict response policies and old-contract/new-server conformance.
- `spline::api!` application composition, shared schemas/configuration, grouped
  clients, and cross-resource namespace/schema/route checks.
- A multi-crate blog tutorial with matching Rust and TypeScript workflows.
- Pinned toolchains, artifact-freshness checks, cross-language tests, contributor
  guidance, GitHub issue/PR templates, and MIT OR Apache-2.0 licensing.
- Reproducible metadata, TypeScript-export, loopback HTTP, and isolated TypeScript
  client benchmarks with fixture checks in CI and documented timing boundaries.
- Protocol-neutral typed Tower operation services and grouped in-process clients,
  including readiness, shared concurrency/backpressure, and cancellation checks.
- A lightweight `spline-http-contract` crate for HTTP descriptions and validation,
  separate from neutral contracts and the Axum/reqwest runtime.
- Neutral definition/service-bundle values and adapter-owned extension traits:
  `.http(policy)`, `.http_with(factory, policy)`, and `.in_process()`. Resource and
  application adapters preserve concrete service/policy bounds and shared state.
- Adapter entry points use `shared(implementation)` for a reusable instance and
  `contract()` for factory-supplied implementations, clarifying their lifetimes.

### Maintenance

- Consolidated runtime and exporter prototypes into the Cargo workspace.
- Promoted Serde inspection/wire probes into maintained tests and removed obsolete
  standalone manifests, lockfiles, and prototype placeholders. Research notes retain
  the original design rationale and link to current code.

### Changed

- TypeScript exports use short public DTO names and authored parameter names.
  Equivalent directions share one declaration; differing nested wire shapes keep
  `Input`/`Output` distinctions. Collisions require explicit exporter overrides.
- Client signatures reference DTOs directly through readable client/result types;
  generated dispatch wrappers and per-operation type aliases are omitted from
  public exports. Regenerate clients and update older generated-type imports.
- Editor checks now verify signature help, narrowed hovers, and property completion
  against the generated blog client using the TypeScript language service.
- The blog tutorial relies on inferred DTO schema identities, demonstrating clean
  TypeScript names without explicit naming annotations.
- `#[spline::contract]` now exposes generated support in the trait's existing Rust
  module and infers the namespace from that module. Optional `module` preserves
  separate companion generation; optional `namespace` pins logical identities.
  The blog tutorial uses normal `users.rs`/`posts.rs` resource modules with no options.
- Generated operation arguments retain documentation on their schema fields while
  consuming parameter doc attributes before lowering to Rust function signatures.
- HTTP entry points and client/export configuration now use explicit `::http`
  modules. `describe()` and `spline::api!` compose neutral operations independently
  of HTTP routing; bound descriptions contain a separate HTTP plan map.
- HTTP dispatch invokes the same typed Tower adapters used in process. Configured
  operation services can be attached through `resource::http::operation::router`.
- Neutral TypeScript export retains the JSON unit shape; only HTTP-bound export
  gives 204 operations a `void` result.
