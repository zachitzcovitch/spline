use spline::http::{axum::http::StatusCode, ErrorPolicy};
use spline_example_blog_contracts::{PostError, UserError};

/// The application chooses HTTP statuses for its declared errors.
pub struct ErrorStatuses;

impl ErrorPolicy<UserError> for ErrorStatuses {
    fn status(&self, error: &UserError) -> StatusCode {
        match error {
            UserError::NotFound => StatusCode::NOT_FOUND,
        }
    }
}

impl ErrorPolicy<PostError> for ErrorStatuses {
    fn status(&self, error: &PostError) -> StatusCode {
        match error {
            PostError::NotFound => StatusCode::NOT_FOUND,
            PostError::Forbidden => StatusCode::FORBIDDEN,
            PostError::InvalidTitle => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
}
