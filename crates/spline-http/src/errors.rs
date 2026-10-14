use crate::{contract::ErrorMapping, ErrorPolicy, StatusCode};

/// An error type's default HTTP interpretation, usually derived with `http::Error`.
/// Custom policies can override this interpretation for a particular application.
pub trait Error: spline_schema::Schema {
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
