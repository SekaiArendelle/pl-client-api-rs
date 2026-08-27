//! Contract tests against the live Physics-Lab API.
//!
//! These tests intentionally require network access. A failure can indicate
//! either an SDK regression, an upstream API change, or a service outage.

use pl_client_api_rs::{Category, Client, QueryExperimentsOptions};

const PUBLIC_EXPERIMENT_ID: &str = "6317fabebfd18200013c710c";

#[tokio::test]
async fn anonymous_login_works() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new()?;
    let session = client.anonymous_login().await?;

    assert!(!session.auth_code().is_empty());
    assert!(!session.current_user().id.is_empty());

    Ok(())
}

#[tokio::test]
async fn get_experiment_works() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new()?;
    let session = client.anonymous_login().await?;
    let response = session
        .get_experiment(PUBLIC_EXPERIMENT_ID, Category::Experiment)
        .await?;

    assert_eq!(
        response.get("Status").and_then(|value| value.as_i64()),
        Some(200)
    );
    assert!(response.get("Data").is_some_and(|data| !data.is_null()));

    Ok(())
}

#[tokio::test]
async fn query_experiments_works() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new()?;
    let session = client.anonymous_login().await?;
    let response = session
        .query_experiments(QueryExperimentsOptions::new(Category::Experiment))
        .await?;

    assert_eq!(
        response.get("Status").and_then(|value| value.as_i64()),
        Some(200)
    );
    assert!(response.get("Data").is_some_and(|data| !data.is_null()));

    Ok(())
}
