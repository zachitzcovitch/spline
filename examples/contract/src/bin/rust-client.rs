use spline::http::CallError;
use spline::Patch;
use spline_example_contract::{users, ApiError, NewUser, ReplaceUser, UserId, UserPatch};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://127.0.0.1:3000".into());
    let client = users::Client::new(&base)?;
    let user = client.get(UserId("user_1".into())).await?;
    println!("Found {}", user.display_name);
    let created = client
        .create(NewUser {
            display_name: "Rust client".into(),
            tags: vec![],
        })
        .await?;
    let fetched = client.get(created.id.clone()).await?;
    assert_eq!(created, fetched);
    println!("Created and fetched {}", created.id.0);
    client
        .replace(
            created.id.clone(),
            ReplaceUser {
                display_name: "Replaced by Rust".into(),
                bio: Some("Before PATCH".into()),
            },
        )
        .await?;
    let updated = client
        .update(
            created.id.clone(),
            UserPatch {
                bio: Patch::Value(None),
                ..UserPatch::default()
            },
        )
        .await?;
    assert_eq!(updated.display_name, "Replaced by Rust");
    assert_eq!(updated.bio, None);
    client.delete(created.id.clone()).await?;
    assert!(matches!(
        client.get(created.id).await,
        Err(CallError::Application(ApiError::NotFound))
    ));
    println!("Replaced, partially updated, and deleted the user");
    match client.get(UserId("missing".into())).await {
        Err(CallError::Application(ApiError::NotFound)) => println!("Handled typed NotFound error"),
        result => return Err(format!("unexpected missing-user response: {result:?}").into()),
    }
    Ok(())
}
