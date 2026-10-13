//! One resource contract, one query, one mutation, and generated TypeScript.

use serde::{Deserialize, Serialize};
use spline::Schema;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(transparent)]
#[spline(name = "example.UserId")]
pub struct UserId(pub String);

/// A public user record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[spline(name = "example.User")]
pub struct User {
    pub id: UserId,
    pub display_name: String,
}

/// Information supplied when creating a user.
#[derive(Debug, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[spline(name = "example.NewUser")]
pub struct NewUser {
    pub display_name: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(
    tag = "kind",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[spline(name = "example.ApiError")]
pub enum ApiError {
    NotFound,
    InvalidName { message: String },
}

/// Operations on user records.
#[spline::contract(namespace = "users", module = "users")]
pub trait Users {
    /// Fetch an existing user.
    #[query]
    #[http(get = "/users/{id}")]
    async fn get(&self, id: UserId) -> Result<User, ApiError>;

    /// Create a user from the supplied profile.
    #[mutation]
    #[http(post = "/users", body = "input", status = 201)]
    async fn create(&self, input: NewUser) -> Result<User, ApiError>;
}

#[derive(Clone)]
pub struct DemoUsers {
    store: std::sync::Arc<tokio::sync::RwLock<Store>>,
}

struct Store {
    next_id: u32,
    users: std::collections::BTreeMap<String, User>,
}

impl Default for DemoUsers {
    fn default() -> Self {
        let user = User {
            id: UserId("user_1".into()),
            display_name: "Zach".into(),
        };
        Self {
            store: std::sync::Arc::new(tokio::sync::RwLock::new(Store {
                next_id: 2,
                users: [(user.id.0.clone(), user)].into_iter().collect(),
            })),
        }
    }
}

impl Users for DemoUsers {
    async fn get(&self, id: UserId) -> Result<User, ApiError> {
        self.store
            .read()
            .await
            .users
            .get(&id.0)
            .cloned()
            .ok_or(ApiError::NotFound)
    }

    async fn create(&self, input: NewUser) -> Result<User, ApiError> {
        if input.display_name.trim().is_empty() {
            return Err(ApiError::InvalidName {
                message: "display name is required".into(),
            });
        }
        let mut store = self.store.write().await;
        let user = User {
            id: UserId(format!("user_{}", store.next_id)),
            display_name: input.display_name,
        };
        store.next_id += 1;
        store.users.insert(user.id.0.clone(), user.clone());
        Ok(user)
    }
}

pub fn error_status(error: &ApiError) -> spline::http::axum::http::StatusCode {
    use spline::http::axum::http::StatusCode;
    match error {
        ApiError::NotFound => StatusCode::NOT_FOUND,
        ApiError::InvalidName { .. } => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

pub fn typescript() -> Result<String, Box<dyn std::error::Error>> {
    let description = users::describe()?;
    Ok(spline_typescript::Typescript::new()
        .export_http_client(&description, "Users")?
        .source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn generated_inputs_decode_and_the_server_trait_is_implementable() {
        let service = DemoUsers::default();
        let get: users::get::Input = serde_json::from_str(r#"{"id":"user_1"}"#).unwrap();
        assert_eq!(service.get(get.id).await.unwrap().display_name, "Zach");
        assert_eq!(
            service.get(UserId("missing".into())).await,
            Err(ApiError::NotFound)
        );
        let create: users::create::Input =
            serde_json::from_str(r#"{"input":{"displayName":"New user"}}"#).unwrap();
        assert!(create.input.tags.is_empty());
        assert_eq!(
            service.create(create.input).await.unwrap().display_name,
            "New user"
        );
        assert!(
            serde_json::from_str::<users::get::Input>(r#"{"id":"user_1","extra":true}"#).is_err()
        );
        assert!(serde_json::from_str::<users::create::Input>(r#"{}"#).is_err());
    }

    #[test]
    fn metadata_and_artifact_have_one_query_and_one_mutation() {
        let description = users::describe().unwrap();
        assert_eq!(description.namespace(), "users");
        assert_eq!(description.operations().len(), 2);
        assert_eq!(
            description.operations()["get"].kind,
            spline::contract::OperationKind::Query
        );
        assert_eq!(
            description.operations()["create"].kind,
            spline::contract::OperationKind::Mutation
        );
        assert_eq!(description.operations()["get"].id, "users.get");
        assert_eq!(
            typescript().unwrap(),
            include_str!("../typescript/generated.ts")
        );
    }
}
