use spline::http::CallError;
use spline_example_contract::{users, ApiError, NewUser, UserId};

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
    match client.get(UserId("missing".into())).await {
        Err(CallError::Application(ApiError::NotFound)) => println!("Handled typed NotFound error"),
        result => return Err(format!("unexpected missing-user response: {result:?}").into()),
    }
    Ok(())
}
