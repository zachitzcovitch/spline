mod posts;
mod users;

use std::collections::BTreeMap;
use std::sync::Arc;

use spline_example_blog_contracts::{Post, User};
use tokio::sync::RwLock;

/// Shared for the lifetime of the server. Restarting the process resets the blog.
pub struct Store {
    users: BTreeMap<String, User>,
    posts: BTreeMap<String, Post>,
}

impl Store {
    pub fn seeded() -> Self {
        let users = [("alice", "Alice"), ("bob", "Bob")]
            .into_iter()
            .map(|(id, name)| {
                (
                    id.into(),
                    User {
                        id: id.into(),
                        display_name: name.into(),
                    },
                )
            })
            .collect();
        Self {
            users,
            posts: BTreeMap::new(),
        }
    }
}

/// One lightweight session per request implements both resource traits.
pub struct Session {
    pub store: Arc<RwLock<Store>>,
    pub user_id: &'static str,
}
