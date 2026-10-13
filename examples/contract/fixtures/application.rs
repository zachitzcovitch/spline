//! Multi-resource conformance fixture. The introductory application lives in
//! examples/blog; this fixture covers composition and protocol edge cases.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use spline::{
    http::{axum::http::StatusCode, ErrorPolicy},
    Schema,
};
use tokio::sync::RwLock;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[spline(name = "composition.User")]
pub struct User {
    pub id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(deny_unknown_fields)]
#[spline(name = "composition.Post")]
pub struct Post {
    pub id: String,
    pub author: User,
    pub title: String,
    pub published: bool,
}

#[derive(Serialize, Deserialize, Schema)]
#[serde(deny_unknown_fields)]
#[spline(name = "composition.NewPost")]
pub struct NewPost {
    pub title: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[spline(name = "composition.UserError")]
pub enum UserError {
    NotFound,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[spline(name = "composition.PostError")]
pub enum PostError {
    NotFound,
    Forbidden,
    InvalidTitle,
}

/// User operations in the shared application.
#[spline::contract(namespace = "app.users", module = "users")]
pub trait Users {
    #[query]
    #[http(get = "/users/{id}")]
    async fn get(&self, id: String) -> Result<User, UserError>;

    #[mutation]
    #[http(post = "/users/me/rename", body = "name")]
    async fn rename(&self, name: String) -> Result<User, UserError>;
}

/// Post operations use the same authenticated session and user store.
#[spline::contract(namespace = "app.posts", module = "posts")]
pub trait Posts {
    #[query]
    #[http(get = "/posts/{id}")]
    async fn get(&self, id: String) -> Result<Post, PostError>;

    #[mutation]
    #[http(post = "/posts", body = "input", status = 201)]
    async fn create(&self, input: NewPost) -> Result<Post, PostError>;

    #[mutation]
    #[http(post = "/posts/{id}/publish")]
    async fn publish(&self, id: String) -> Result<Post, PostError>;

    #[mutation]
    #[http(delete = "/posts/{id}", status = 204)]
    async fn delete(&self, id: String) -> Result<(), PostError>;
}

spline::api! {
    /// Users and posts, composed with shared configuration.
    pub mod api {
        users: self::users,
        posts: self::posts,
    }
}

pub fn typescript() -> Result<String, Box<dyn std::error::Error>> {
    Ok(spline_typescript::Typescript::new()
        .export_http_application(&api::describe()?, "Api")?
        .source)
}

struct StoredPost {
    author: String,
    title: String,
    published: bool,
}
struct Store {
    users: BTreeMap<String, User>,
    posts: BTreeMap<String, StoredPost>,
    next_id: u32,
}

impl Store {
    fn post(&self, id: &str) -> Result<Post, PostError> {
        let post = self.posts.get(id).ok_or(PostError::NotFound)?;
        Ok(Post {
            id: id.into(),
            author: self.users[&post.author].clone(),
            title: post.title.clone(),
            published: post.published,
        })
    }
}

#[derive(Clone)]
pub struct Database(Arc<RwLock<Store>>);

impl Default for Database {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(Store {
            users: ["alice", "bob"]
                .map(|id| {
                    (
                        id.into(),
                        User {
                            id: id.into(),
                            display_name: id.into(),
                        },
                    )
                })
                .into(),
            posts: BTreeMap::new(),
            next_id: 0,
        })))
    }
}

pub struct Session {
    pub database: Database,
    pub principal: String,
}

impl Users for Session {
    async fn get(&self, id: String) -> Result<User, UserError> {
        self.database
            .0
            .read()
            .await
            .users
            .get(&id)
            .cloned()
            .ok_or(UserError::NotFound)
    }

    async fn rename(&self, name: String) -> Result<User, UserError> {
        let mut store = self.database.0.write().await;
        let user = store
            .users
            .get_mut(&self.principal)
            .ok_or(UserError::NotFound)?;
        user.display_name = name;
        Ok(user.clone())
    }
}

impl Posts for Session {
    async fn get(&self, id: String) -> Result<Post, PostError> {
        self.database.0.read().await.post(&id)
    }

    async fn create(&self, input: NewPost) -> Result<Post, PostError> {
        if input.title.trim().is_empty() {
            return Err(PostError::InvalidTitle);
        }
        let mut store = self.database.0.write().await;
        if !store.users.contains_key(&self.principal) {
            return Err(PostError::Forbidden);
        }
        store.next_id += 1;
        let id = format!("post_{}", store.next_id);
        store.posts.insert(
            id.clone(),
            StoredPost {
                author: self.principal.clone(),
                title: input.title,
                published: false,
            },
        );
        store.post(&id)
    }

    async fn publish(&self, id: String) -> Result<Post, PostError> {
        let mut store = self.database.0.write().await;
        let post = store.posts.get_mut(&id).ok_or(PostError::NotFound)?;
        if post.author != self.principal {
            return Err(PostError::Forbidden);
        }
        post.published = true;
        store.post(&id)
    }

    async fn delete(&self, id: String) -> Result<(), PostError> {
        let mut store = self.database.0.write().await;
        let post = store.posts.get(&id).ok_or(PostError::NotFound)?;
        if post.author != self.principal {
            return Err(PostError::Forbidden);
        }
        store.posts.remove(&id);
        Ok(())
    }
}

// One policy can cover distinct error types without requiring Clone.
pub struct Errors;
impl ErrorPolicy<UserError> for Errors {
    fn status(&self, _: &UserError) -> StatusCode {
        StatusCode::NOT_FOUND
    }
}
impl ErrorPolicy<PostError> for Errors {
    fn status(&self, error: &PostError) -> StatusCode {
        match error {
            PostError::NotFound => StatusCode::NOT_FOUND,
            PostError::Forbidden => StatusCode::FORBIDDEN,
            PostError::InvalidTitle => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
}
