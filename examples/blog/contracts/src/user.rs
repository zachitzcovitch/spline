use serde::{Deserialize, Serialize};
use spline::Schema;

#[derive(Clone, Debug, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[spline(name = "blog.User")]
pub struct User {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Serialize, Deserialize, Schema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[spline(name = "blog.UserError")]
pub enum UserError {
    NotFound,
}

/// Profiles shared by blog authors and readers.
#[spline::contract(namespace = "blog.users", module = "users")]
pub trait Users {
    /// Look up a profile by its ID.
    #[query]
    #[http(get = "/users/{id}")]
    async fn get(&self, id: String) -> Result<User, UserError>;
}
