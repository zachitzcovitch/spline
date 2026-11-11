//! Explicit HTTP/JSON execution for Splinekit contracts.

#![forbid(unsafe_code)]

#[cfg(feature = "client")]
mod client;
mod compatibility;
mod errors;
#[cfg(feature = "server")]
mod extensions;
#[cfg(feature = "client")]
mod response;
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "client")]
pub mod transport;

#[cfg(feature = "server")]
pub use axum;
#[cfg(feature = "client")]
pub use client::Client;
pub use compatibility::verify_serde;
pub use errors::{DeclaredErrors, Error, ErrorPolicy, Fault};
#[cfg(feature = "server")]
pub use extensions::{HttpExt, HttpFactoryBinding, HttpFactoryExt, HttpServiceBinding};
pub use http::StatusCode;
#[cfg(feature = "client")]
pub use reqwest;
#[cfg(feature = "client")]
pub use response::ResponsePolicy;
#[cfg(feature = "server")]
pub use server::RequestContext;
pub use splinekit_http_contract as contract;
pub use splinekit_macros::HttpError as Error;
#[cfg(feature = "client")]
pub use transport::{HttpRequest, HttpResponse, ReqwestTransport, Transport};

use std::fmt;

pub use splinekit_http_contract::{ERROR_HEADER, MAX_BODY_BYTES};

mod sealed {
    pub trait Unit {}
    impl Unit for () {}
}

/// A 204 operation has no success payload. Only Rust unit implements this trait;
/// aliases of () work, while nullable or custom null-encoded DTOs do not qualify.
pub trait NoContent: sealed::Unit {}
impl NoContent for () {}

#[derive(Debug)]
pub enum BuildError {
    Contract(splinekit_contract::Error),
    Binding(splinekit_http_contract::Error),
    Schema(splinekit_schema::Error),
    Configuration(String),
}

impl From<splinekit_contract::Error> for BuildError {
    fn from(value: splinekit_contract::Error) -> Self {
        Self::Contract(value)
    }
}
impl From<splinekit_http_contract::Error> for BuildError {
    fn from(value: splinekit_http_contract::Error) -> Self {
        Self::Binding(value)
    }
}
impl From<splinekit_schema::Error> for BuildError {
    fn from(value: splinekit_schema::Error) -> Self {
        Self::Schema(value)
    }
}
impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(e) => e.fmt(f),
            Self::Binding(e) => e.fmt(f),
            Self::Schema(e) => e.fmt(f),
            Self::Configuration(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for BuildError {}

#[cfg(feature = "client")]
#[derive(Debug)]
pub enum CallError<E> {
    Application(E),
    Framework(Fault),
    Http {
        status: u16,
    },
    /// The transport failed to send the request or read the response.
    Transport(tower::BoxError),
    Protocol(String),
    Encode(String),
}

#[cfg(feature = "client")]
impl<E: fmt::Debug> fmt::Display for CallError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application(error) => write!(f, "application error: {error:?}"),
            Self::Framework(error) => write!(f, "{}: {}", error.code, error.message),
            Self::Http { status } => write!(f, "unrecognized HTTP response: {status}"),
            Self::Transport(error) => error.fmt(f),
            Self::Protocol(message) | Self::Encode(message) => f.write_str(message),
        }
    }
}
#[cfg(feature = "client")]
impl<E: fmt::Debug> std::error::Error for CallError<E> {}

#[cfg(any(feature = "client", feature = "server"))]
pub(crate) fn json_content_type(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        let value = value
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        value == "application/json"
            || (value.starts_with("application/") && value.ends_with("+json"))
    })
}
