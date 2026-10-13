use spline_example_blog_contracts::{User, UserError, Users};

use super::Session;

impl Users for Session {
    async fn get(&self, id: String) -> Result<User, UserError> {
        self.store
            .read()
            .await
            .users
            .get(&id)
            .cloned()
            .ok_or(UserError::NotFound)
    }
}
