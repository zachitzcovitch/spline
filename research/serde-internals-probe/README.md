# Serde internals reuse probe

An isolated experiment for the [reuse evaluation](../serde-internals-reuse.md).
It parses Serde attributes through a private adapter and compares the resulting
metadata with actual Serde behavior.

## Layout

- `src/adapter.rs`: all Serde-internals and Syn integration.
- `src/model.rs`: owned inspection metadata, with no parser dependencies in its types.
- `tests/compatibility.rs`: metadata, wire-behavior, and diagnostic comparisons.
- `tests/ui/`: real compiler failures that expose the boundaries of parser validation.

Most behavior fixtures define a Rust type once, using the same tokens for the
inspection input and actual Serde derives. Metadata covers the attribute families
under evaluation; inspection success is not a declaration of Spline schema support.

The type/path strings are diagnostic syntax. A production derive would retain
Rust types and spans on the macro side and emit typed schema references.

## Run

From the repository root:

```sh
cargo test --locked --manifest-path research/serde-internals-probe/Cargo.toml
cargo fmt --manifest-path research/serde-internals-probe/Cargo.toml -- --check
cargo clippy --locked --manifest-path research/serde-internals-probe/Cargo.toml --all-targets -- -D warnings
```

The lockfile records the evaluated dependencies. Compiler-failure snapshots were
reviewed on rustc 1.95.0-nightly (859951e3c, 2026-02-24); diagnostic changes on other
toolchains may need review.
