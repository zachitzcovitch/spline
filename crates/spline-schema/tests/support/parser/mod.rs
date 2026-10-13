//! Test-only Serde attribute inspection, retained from the original research.
//!
//! All public values are owned descriptions. Serde's unstable AST and Syn types
//! stay inside the adapter. Rust type/path strings are for inspection, not type
//! resolution or persistent schema identity.

#![allow(dead_code)] // Each integration-test binary inspects different metadata.

mod adapter;
mod model;

pub use model::*;

/// Inspect one already-expanded struct or enum declaration in a Serde direction.
///
/// The production proc macro receives tokens from rustc; this owned inspection
/// model is an independent test oracle, not part of Spline's public API.
pub fn inspect(source: &str, direction: Direction) -> Result<Container, Vec<Diagnostic>> {
    adapter::inspect(source, direction)
}
