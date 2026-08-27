use serde::Serialize;
use serde_json::Value;

use crate::client::Client;
use crate::error::Error;
use crate::models::{Category, CurrentUser};

/// Information returned by a successful login.
///
/// A session borrows the client that created it. This keeps their lifetime
/// relationship explicit and lets Rust prevent the client from being dropped
/// while the session is still in use.
///
/// Credentials are intentionally private so they are not printed accidentally.
pub struct Session<'client> {
    client: &'client Client,
    token: Option<String>,
    auth_code: String,
    device_token: Option<String>,
    current_user: CurrentUser,
    statistic: Value,
}

impl<'client> Session<'client> {
    pub(crate) fn new(
        client: &'client Client,
        token: Option<String>,
        auth_code: String,
        device_token: Option<String>,
        current_user: CurrentUser,
        statistic: Value,
    ) -> Self {
        Self {
            client,
            token,
            auth_code,
            device_token,
            current_user,
            statistic,
        }
    }

    pub fn client(&self) -> &'client Client {
        self.client
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn auth_code(&self) -> &str {
        &self.auth_code
    }

    pub fn device_token(&self) -> Option<&str> {
        self.device_token.as_deref()
    }

    pub fn current_user(&self) -> &CurrentUser {
        &self.current_user
    }

    pub fn statistic(&self) -> &Value {
        &self.statistic
    }

    /// Gets an experiment or discussion summary.
    ///
    /// The complete API response is returned because the summary payload has
    /// not been stabilized as a public Rust model yet.
    pub async fn get_summary(&self, content_id: &str, category: Category) -> Result<Value, Error> {
        let request = GetSummaryRequest {
            content_id,
            category,
        };

        let response = self
            .authenticated_post("Contents/GetSummary")
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        check_api_response(&response)?;
        Ok(response)
    }

    /// Gets an experiment from an experiment or discussion identifier.
    ///
    /// This first resolves the identifier through [`Self::get_summary`], then
    /// uses the `ContentID` returned by that endpoint to fetch the experiment.
    pub async fn get_experiment(
        &self,
        experiment_id: &str,
        category: Category,
    ) -> Result<Value, Error> {
        let content_id = {
            let summary = self.get_summary(experiment_id, category).await?;
            summary_content_id(&summary)?.to_owned()
        };

        self.get_experiment_by_content_id(&content_id).await
    }

    /// Gets an experiment when its resolved `ContentID` is already known.
    ///
    /// Most callers should use [`Self::get_experiment`]. This method avoids an
    /// unnecessary summary request when the API-level content identifier is
    /// already available.
    pub async fn get_experiment_by_content_id(&self, content_id: &str) -> Result<Value, Error> {
        let request = GetExperimentRequest { content_id };

        let response = self
            .authenticated_post("Contents/GetExperiment")
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        check_api_response(&response)?;
        Ok(response)
    }

    fn authenticated_post(&self, path: &str) -> reqwest::RequestBuilder {
        self.client
            .post(path)
            .header("x-API-Token", self.token.as_deref().unwrap_or("null"))
            .header("x-API-AuthCode", &self.auth_code)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct GetSummaryRequest<'a> {
    #[serde(rename = "ContentID")]
    content_id: &'a str,
    category: Category,
}

#[derive(Serialize)]
struct GetExperimentRequest<'a> {
    #[serde(rename = "ContentID")]
    content_id: &'a str,
}

fn check_api_response(response: &Value) -> Result<(), Error> {
    let status = response
        .get("Status")
        .and_then(Value::as_i64)
        .ok_or(Error::MissingField("Status"))?;

    if status == 200 {
        return Ok(());
    }

    let message = response
        .get("Message")
        .and_then(Value::as_str)
        .unwrap_or("unknown server error")
        .to_owned();

    Err(Error::Api { status, message })
}

fn summary_content_id(summary: &Value) -> Result<&str, Error> {
    summary
        .pointer("/Data/ContentID")
        .and_then(Value::as_str)
        .ok_or(Error::MissingField("Data.ContentID"))
}
