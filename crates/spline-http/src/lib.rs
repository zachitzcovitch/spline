//! Explicit HTTP/JSON execution for Spline contracts.

#![forbid(unsafe_code)]

mod client;
mod compatibility;
mod errors;
mod extensions;
mod response;
pub mod server;

pub use axum;
pub use axum::http::StatusCode;
pub use client::Client;
pub use compatibility::verify_serde;
pub use errors::{DeclaredErrors, Error};
pub use extensions::{HttpExt, HttpFactoryBinding, HttpFactoryExt, HttpServiceBinding};
pub use reqwest;
pub use response::ResponsePolicy;
pub use server::{ErrorPolicy, Fault, RequestContext};
pub use spline_http_contract as contract;
pub use spline_macros::HttpError as Error;

use std::fmt;

pub use spline_http_contract::ERROR_HEADER;
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

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
    Contract(spline_contract::Error),
    Binding(spline_http_contract::Error),
    Schema(spline_schema::Error),
    Configuration(String),
}

impl From<spline_contract::Error> for BuildError {
    fn from(value: spline_contract::Error) -> Self {
        Self::Contract(value)
    }
}
impl From<spline_http_contract::Error> for BuildError {
    fn from(value: spline_http_contract::Error) -> Self {
        Self::Binding(value)
    }
}
impl From<spline_schema::Error> for BuildError {
    fn from(value: spline_schema::Error) -> Self {
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

#[derive(Debug)]
pub enum CallError<E> {
    Application(E),
    Framework(Fault),
    Http { status: u16 },
    Transport(reqwest::Error),
    Protocol(String),
    Encode(String),
}

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
impl<E: fmt::Debug> std::error::Error for CallError<E> {}

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
