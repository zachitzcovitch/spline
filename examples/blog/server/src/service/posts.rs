use spline_example_blog_contracts::{NewPost, Post, PostError, Posts};

use super::Session;

impl Posts for Session {
    async fn get(&self, id: String) -> Result<Post, PostError> {
        self.store
            .read()
            .await
            .posts
            .get(&id)
            .cloned()
            .ok_or(PostError::NotFound)
    }

    async fn create(&self, input: NewPost) -> Result<Post, PostError> {
        let title = input.title.trim();
        if title.is_empty() {
            return Err(PostError::InvalidTitle);
        }

        let mut store = self.store.write().await;
        let author = store
            .users
            .get(self.user_id)
            .cloned()
            .ok_or(PostError::Forbidden)?;
        // Posts are append-only in this example, so the count supplies a unique ID.
        let id = format!("post_{}", store.posts.len() + 1);
        let post = Post {
            id: id.clone(),
            author,
            title: title.into(),
            body: input.body,
            published: false,
        };
        store.posts.insert(id, post.clone());
        Ok(post)
    }

    async fn publish(&self, id: String) -> Result<Post, PostError> {
        let mut store = self.store.write().await;
        let post = store.posts.get_mut(&id).ok_or(PostError::NotFound)?;
        if post.author.id != self.user_id {
            return Err(PostError::Forbidden);
        }
        post.published = true;
        Ok(post.clone())
    }
}
