//! The shared blog contract crate. The server and Rust client depend on this
//! library; the export example generates the TypeScript client from it.

pub mod posts;
pub mod users;

pub use posts::{NewPost, Post, PostError, Posts};
pub use users::{User, UserError, Users};

spline::api! {
    /// One application with grouped clients and a shared server context.
    pub mod api {
        users: crate::users,
        posts: crate::posts,
    }
}
