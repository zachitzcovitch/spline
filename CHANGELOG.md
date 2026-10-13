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

### Maintenance

- Consolidated runtime and exporter prototypes into the Cargo workspace.
- Promoted Serde inspection/wire probes into maintained tests and removed obsolete
  standalone manifests, lockfiles, and prototype placeholders. Research notes retain
  the original design rationale and link to current code.
