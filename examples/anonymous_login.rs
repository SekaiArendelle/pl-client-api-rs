use pl_client_api_rs::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new()?;
    let session = client.anonymous_login().await?;
    let user = session.current_user();

    println!(
        "logged in as: {}",
        user.nickname.as_deref().unwrap_or("<anonymous>")
    );
    println!("user id: {}", user.id);
    println!("level: {}", user.level);

    Ok(())
}
