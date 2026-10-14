use serde::{Deserialize, Serialize};
use spline::http::StatusCode;
use spline::Schema;

use crate::User;

#[derive(Clone, Debug, Serialize, Deserialize, Schema)]
#[serde(deny_unknown_fields)]
pub struct Post {
    pub id: String,
    pub author: User,
    pub title: String,
    pub body: String,
    pub published: bool,
}

/// The author comes from the authenticated session, so clients supply only content.
#[derive(Serialize, Deserialize, Schema)]
#[serde(deny_unknown_fields)]
pub struct NewPost {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Serialize, Deserialize, Schema, spline::http::Error)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PostError {
    #[http(status = StatusCode::NOT_FOUND)]
    NotFound,
    #[http(status = StatusCode::FORBIDDEN)]
    Forbidden,
    #[http(status = StatusCode::UNPROCESSABLE_ENTITY)]
    InvalidTitle,
}

/// Drafts and publishing operations.
#[spline::contract]
pub trait Posts {
    /// Read a post. Authenticated readers can view drafts as well as published posts.
    #[query]
    #[http(get = "/posts/{id}")]
    async fn get(&self, id: String) -> Result<Post, PostError>;

    /// Create a draft owned by the authenticated user. The title must not be blank.
    #[mutation]
    #[http(post = "/posts", body = "input", status = 201)]
    async fn create(&self, input: NewPost) -> Result<Post, PostError>;

    /// Publish a draft. Only its author may publish it; repeating the call is allowed.
    #[mutation]
    #[http(post = "/posts/{id}/publish")]
    async fn publish(&self, id: String) -> Result<Post, PostError>;
}
