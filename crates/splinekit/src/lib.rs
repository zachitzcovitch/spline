//! Rust-authored contract and schema APIs.

extern crate self as splinekit;

pub mod codec;
mod patch;

pub use patch::Patch;
pub use splinekit_contract as contract;
pub use splinekit_macros::{api, contract, Schema};
pub use splinekit_runtime as runtime;
pub use splinekit_schema::*;

#[cfg(feature = "http")]
pub use splinekit_http as http;

#[doc(hidden)]
pub mod __private {
    pub use crate::{__splinekit_http as http, __splinekit_http_input as http_input};
    pub use serde;
    pub const HTTP_ENABLED: bool = cfg!(feature = "http");

    pub fn assert_input<T: serde::de::DeserializeOwned + Send>() {}
    pub fn assert_output<T: serde::Serialize + Send>() {}

    #[cfg(feature = "http")]
    pub fn assert_no_content<T: splinekit_http::NoContent>() {}
}

#[doc(hidden)]
#[cfg(feature = "http")]
#[macro_export]
macro_rules! __splinekit_http { ($($tokens:tt)*) => { $($tokens)* }; }
#[doc(hidden)]
#[cfg(not(feature = "http"))]
#[macro_export]
macro_rules! __splinekit_http {
    ($($tokens:tt)*) => {};
}

#[doc(hidden)]
#[cfg(feature = "http")]
#[macro_export]
macro_rules! __splinekit_http_input {
    ($http:item, $neutral:item) => {
        $http
    };
}
#[doc(hidden)]
#[cfg(not(feature = "http"))]
#[macro_export]
macro_rules! __splinekit_http_input {
    ($http:item, $neutral:item) => {
        $neutral
    };
}
