//! Rust-authored contract and schema APIs.

extern crate self as spline;

mod patch;

pub use patch::Patch;
pub use spline_contract as contract;
pub use spline_macros::{api, contract, Schema};
pub use spline_runtime as runtime;
pub use spline_schema::*;

#[cfg(feature = "http")]
pub use spline_http as http;

#[doc(hidden)]
pub mod __private {
    pub use crate::{__spline_http as http, __spline_http_input as http_input};
    pub use serde;
    pub const HTTP_ENABLED: bool = cfg!(feature = "http");

    pub fn assert_input<T: serde::de::DeserializeOwned + Send>() {}
    pub fn assert_output<T: serde::Serialize + Send>() {}

    #[cfg(feature = "http")]
    pub fn assert_no_content<T: spline_http::NoContent>() {}
}

#[doc(hidden)]
#[cfg(feature = "http")]
#[macro_export]
macro_rules! __spline_http { ($($tokens:tt)*) => { $($tokens)* }; }
#[doc(hidden)]
#[cfg(not(feature = "http"))]
#[macro_export]
macro_rules! __spline_http {
    ($($tokens:tt)*) => {};
}

#[doc(hidden)]
#[cfg(feature = "http")]
#[macro_export]
macro_rules! __spline_http_input {
    ($http:item, $neutral:item) => {
        $http
    };
}
#[doc(hidden)]
#[cfg(not(feature = "http"))]
#[macro_export]
macro_rules! __spline_http_input {
    ($http:item, $neutral:item) => {
        $neutral
    };
}
