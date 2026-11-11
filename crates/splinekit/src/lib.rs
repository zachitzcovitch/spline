//! Rust-authored contract and schema APIs.

extern crate self as splinekit;

pub mod codec;
mod open;
mod patch;

pub use open::Open;
pub use patch::Patch;
pub use splinekit_contract as contract;
pub use splinekit_macros::{api, contract, Schema};
pub use splinekit_runtime as runtime;
pub use splinekit_schema::*;

#[cfg(any(feature = "http-client", feature = "http-server"))]
pub use splinekit_http as http;

#[doc(hidden)]
pub mod __private {
    pub use crate::{
        __splinekit_http as http, __splinekit_http_client as http_client,
        __splinekit_http_input as http_input, __splinekit_http_server as http_server,
    };
    pub use serde;
    pub const HTTP_ENABLED: bool = cfg!(any(feature = "http-client", feature = "http-server"));

    pub fn assert_input<T: serde::de::DeserializeOwned + Send>() {}
    pub fn assert_output<T: serde::Serialize + Send>() {}

    #[cfg(any(feature = "http-client", feature = "http-server"))]
    pub fn assert_no_content<T: splinekit_http::NoContent>() {}
}

// Generated code wraps its HTTP items in these, so the binding a build selects
// (client, server or both) decides what exists. Cargo unifies features per
// build: a contracts crate shared by a server and a wasm32 client generates the
// router only in the server's build.

/// Items for any HTTP binding.
#[doc(hidden)]
#[cfg(any(feature = "http-client", feature = "http-server"))]
#[macro_export]
macro_rules! __splinekit_http { ($($tokens:tt)*) => { $($tokens)* }; }
#[doc(hidden)]
#[cfg(not(any(feature = "http-client", feature = "http-server")))]
#[macro_export]
macro_rules! __splinekit_http {
    ($($tokens:tt)*) => {};
}

/// Items for the generated HTTP client.
#[doc(hidden)]
#[cfg(feature = "http-client")]
#[macro_export]
macro_rules! __splinekit_http_client { ($($tokens:tt)*) => { $($tokens)* }; }
#[doc(hidden)]
#[cfg(not(feature = "http-client"))]
#[macro_export]
macro_rules! __splinekit_http_client {
    ($($tokens:tt)*) => {};
}

/// Items for the Axum server.
#[doc(hidden)]
#[cfg(feature = "http-server")]
#[macro_export]
macro_rules! __splinekit_http_server { ($($tokens:tt)*) => { $($tokens)* }; }
#[doc(hidden)]
#[cfg(not(feature = "http-server"))]
#[macro_export]
macro_rules! __splinekit_http_server {
    ($($tokens:tt)*) => {};
}

#[doc(hidden)]
#[cfg(any(feature = "http-client", feature = "http-server"))]
#[macro_export]
macro_rules! __splinekit_http_input {
    ($http:item, $neutral:item) => {
        $http
    };
}
#[doc(hidden)]
#[cfg(not(any(feature = "http-client", feature = "http-server")))]
#[macro_export]
macro_rules! __splinekit_http_input {
    ($http:item, $neutral:item) => {
        $neutral
    };
}
