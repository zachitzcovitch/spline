use serde::{Deserialize, Serialize};
use splinekit::http::StatusCode;
use splinekit::Schema;

#[derive(Clone, Debug, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct User {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Serialize, Deserialize, Schema, splinekit::http::Error)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UserError {
    #[http(status = StatusCode::NOT_FOUND)]
    NotFound,
}

/// Profiles shared by blog authors and readers.
#[splinekit::contract]
pub trait Users {
    /// Look up a profile by its ID.
    #[query]
    #[http(get = "/users/{id}")]
    async fn get(&self, id: String) -> Result<User, UserError>;
}
