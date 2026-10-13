//! Independent contract revisions for compatibility tests. Keep v1's contract
//! fixed when extending v2; both intentionally use the same logical wire names.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use spline::Schema;

pub mod v1 {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.Profile")]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Profile {
        pub display_name: String,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.State")]
    #[serde(rename_all = "snake_case")]
    pub enum State {
        Active,
        Inactive,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.User")]
    #[serde(deny_unknown_fields)]
    pub struct User {
        pub id: String,
        pub profile: Profile,
        pub history: Vec<Profile>,
        pub profiles: BTreeMap<String, Profile>,
        pub counters: BTreeMap<String, u32>,
        pub nickname: Option<String>,
        pub state: State,
    }

    #[derive(Debug, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.Create")]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Create {
        pub display_name: String,
        pub state: State,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.Error")]
    #[serde(
        tag = "kind",
        content = "details",
        rename_all = "snake_case",
        deny_unknown_fields
    )]
    pub enum ApiError {
        Missing { message: String },
        Rejected,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.Event")]
    #[serde(rename_all = "snake_case", deny_unknown_fields)]
    pub enum Event {
        Item { id: String },
        Other { id: String },
    }

    #[spline::contract(namespace = "evolution.users", module = "users")]
    pub trait Users {
        #[query]
        #[http(get = "/evolution/users/{id}")]
        async fn get(&self, id: String) -> Result<User, ApiError>;

        #[mutation]
        #[http(post = "/evolution/users", body = "input", status = 201)]
        async fn create(&self, input: Create) -> Result<User, ApiError>;

        #[query]
        #[http(get = "/evolution/events/{id}")]
        async fn event(&self, id: String) -> Result<Event, ApiError>;
    }
}

pub mod v2 {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.Profile")]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Profile {
        pub display_name: String,
        pub locale: String,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.State")]
    #[serde(rename_all = "snake_case")]
    pub enum State {
        Active,
        Inactive,
        Suspended,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.User")]
    #[serde(deny_unknown_fields)]
    pub struct User {
        pub id: String,
        pub profile: Profile,
        pub history: Vec<Profile>,
        pub profiles: BTreeMap<String, Profile>,
        pub counters: BTreeMap<String, u32>,
        pub nickname: Option<String>,
        pub state: State,
        pub revision: u32,
    }

    #[derive(Debug, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.Create")]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Create {
        pub display_name: String,
        pub state: State,
        #[serde(default)]
        pub tags: Vec<String>,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.Error")]
    #[serde(
        tag = "kind",
        content = "details",
        rename_all = "snake_case",
        deny_unknown_fields
    )]
    pub enum ApiError {
        Missing { message: String, retryable: bool },
        Rejected,
        Busy { retry_after: u32 },
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.Event")]
    #[serde(rename_all = "snake_case", deny_unknown_fields)]
    pub enum Event {
        Item { id: String, revision: u32 },
        Other { id: String },
        Removed { id: String },
    }

    #[spline::contract(namespace = "evolution.users", module = "users")]
    pub trait Users {
        #[query]
        #[http(get = "/evolution/users/{id}")]
        async fn get(&self, id: String, audit: Option<String>) -> Result<User, ApiError>;

        #[mutation]
        #[http(post = "/evolution/users", body = "input", status = 201)]
        async fn create(&self, input: Create) -> Result<User, ApiError>;

        #[query]
        #[http(get = "/evolution/events/{id}")]
        async fn event(&self, id: String) -> Result<Event, ApiError>;
    }
}

/// A separate negative fixture: a new required request field is a breaking change.
pub mod required_input {
    use super::*;

    #[derive(Debug, Serialize, Deserialize, Schema)]
    #[spline(name = "evolution.RequiredCreate")]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Create {
        pub display_name: String,
        pub state: v2::State,
        pub approval: String,
    }

    #[spline::contract(namespace = "evolution.users", module = "users")]
    pub trait Users {
        #[mutation]
        #[http(post = "/evolution/users", body = "input", status = 201)]
        async fn create(&self, input: Create) -> Result<v2::User, v2::ApiError>;
    }
}

pub fn typescript() -> Result<String, Box<dyn std::error::Error>> {
    Ok(spline_typescript::Typescript::new()
        .export_http_client(&v1::users::describe()?, "EvolutionV1")?
        .source)
}
