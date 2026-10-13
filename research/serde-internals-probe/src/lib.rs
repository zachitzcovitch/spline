//! An attribute-inspection experiment, not a schema implementation.
//!
//! All public values are owned descriptions. Serde's unstable AST and Syn types
//! stay inside the adapter. Rust type/path strings are for inspection, not type
//! resolution or persistent schema identity.

#![forbid(unsafe_code)]

mod adapter;
mod model;

pub use model::*;

/// Inspect one already-expanded struct or enum declaration in a Serde direction.
///
/// The string interface makes the research fixtures convenient. A production
/// proc macro would receive tokens from rustc and retain spans for diagnostics.
pub fn inspect(source: &str, direction: Direction) -> Result<Container, Vec<Diagnostic>> {
    adapter::inspect(source, direction)
}
