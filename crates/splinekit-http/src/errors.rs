use crate::{contract::ErrorMapping, StatusCode};

/// An error type's default HTTP interpretation, usually derived with `http::Error`.
/// Custom policies can override this interpretation for a particular application.
pub trait Error: splinekit_schema::Schema {
    fn status(&self) -> StatusCode;
    fn mapping() -> ErrorMapping;
}

/// Use each error type's declared default statuses and response metadata.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeclaredErrors;

impl<E: Error> ErrorPolicy<E> for DeclaredErrors {
    fn status(&self, error: &E) -> StatusCode {
        error.status()
    }

    fn mapping(&self) -> ErrorMapping {
        E::mapping()
    }
}

/// Maps an application error to its HTTP status. Lives outside the server so
/// clients can describe the same responses.
pub trait ErrorPolicy<E>: Send + Sync + 'static {
    fn status(&self, error: &E) -> StatusCode;

    /// Static application-error mappings, when known. Must describe all emitted
    /// application values/statuses for this configuration. Return Opaque when a
    /// dynamic policy cannot declare its responses exhaustively.
    fn mapping(&self) -> ErrorMapping {
        ErrorMapping::Opaque
    }
}
impl<E, F> ErrorPolicy<E> for F
where
    F: Fn(&E) -> StatusCode + Send + Sync + 'static,
{
    fn status(&self, error: &E) -> StatusCode {
        self(error)
    }
}

/// A framework error: the request or the service failed outside the
/// application's own errors. Servers send it as problem details; clients
/// decode it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    pub status: u16,
    /// A stable, machine-readable code. Branch on this.
    pub code: String,
    /// Developer-facing text for logs and debugging: not for end users, and
    /// free to change.
    pub detail: Option<String>,
}

impl Fault {
    pub fn new(status: u16, code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            detail: Some(detail.into()),
        }
    }
    pub fn invalid(detail: impl Into<String>) -> Self {
        Self::new(400, "invalid_request", detail)
    }
    #[cfg(feature = "server")]
    pub(crate) fn internal() -> Self {
        Self::new(500, "internal", "response encoding or error policy failed")
    }
}
