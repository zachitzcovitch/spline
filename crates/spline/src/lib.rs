//! Rust-authored contract and schema APIs.

extern crate self as spline;

pub use spline_contract as contract;
pub use spline_macros::{contract, Schema};
pub use spline_schema::*;

#[cfg(feature = "http")]
pub use spline_http as http;

#[doc(hidden)]
pub mod __private {
    pub use serde;

    pub fn assert_input<T: serde::de::DeserializeOwned + Send>() {}
    pub fn assert_output<T: serde::Serialize + Send>() {}
}
