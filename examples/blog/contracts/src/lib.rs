//! The shared blog contract crate. The server and Rust client depend on this
//! library; the export example generates the TypeScript client from it.

mod post;
mod user;

pub use post::{posts, NewPost, Post, PostError, Posts};
pub use user::{users, User, UserError, Users};

spline::api! {
    /// One application with grouped clients and a shared server context.
    pub mod api {
        users: crate::users,
        posts: crate::posts,
    }
}
