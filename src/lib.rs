use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// An asynchronous client for the Physics-Lab API.
pub struct Client {
    http: reqwest::Client,
    api_base_url: reqwest::Url,
    plar_version: u32,
    device_id: String,
    language: String,
}

/// Configures and constructs a [`Client`].
pub struct ClientBuilder {
    api_base_url: String,
    request_timeout: Duration,
    plar_version: u32,
    device_id: String,
    language: String,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            api_base_url: "https://physics-api-cn.turtlesim.com".to_owned(),
            request_timeout: Duration::from_secs(15),
            plar_version: 2411,
            device_id: "7db01528cf13e2199e141c402d79190e".to_owned(),
            language: "Chinese".to_owned(),
        }
    }
}

impl ClientBuilder {
    /// Overrides the Physics-Lab API base URL.
    ///
    /// A custom URL is useful when running integration tests against a local
    /// mock server. It is parsed and validated by [`Self::build`].
    pub fn api_base_url(mut self, api_base_url: impl Into<String>) -> Self {
        self.api_base_url = api_base_url.into();
        self
    }

    /// Sets the total timeout for each HTTP request.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Sets the Physics-Lab client version sent during authentication.
    pub fn plar_version(mut self, version: u32) -> Self {
        self.plar_version = version;
        self
    }

    /// Sets the device identifier sent during authentication.
    pub fn device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = device_id.into();
        self
    }

    /// Sets the language sent during authentication.
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = language.into();
        self
    }

    /// Validates the configuration and constructs a client.
    pub fn build(self) -> Result<Client, Error> {
        let mut api_base_url =
            reqwest::Url::parse(&self.api_base_url).map_err(|error| Error::InvalidBaseUrl {
                value: self.api_base_url.clone(),
                reason: error.to_string(),
            })?;

        if !matches!(api_base_url.scheme(), "http" | "https")
            || api_base_url.cannot_be_a_base()
            || api_base_url.host_str().is_none()
        {
            return Err(Error::InvalidBaseUrl {
                value: self.api_base_url,
                reason: "expected an absolute HTTP or HTTPS URL with a host".to_owned(),
            });
        }

        if api_base_url.query().is_some() || api_base_url.fragment().is_some() {
            return Err(Error::InvalidBaseUrl {
                value: self.api_base_url,
                reason: "query strings and fragments are not allowed".to_owned(),
            });
        }

        if !api_base_url.path().ends_with('/') {
            let mut path = api_base_url.path().to_owned();
            path.push('/');
            api_base_url.set_path(&path);
        }

        let http = reqwest::Client::builder()
            .timeout(self.request_timeout)
            .build()?;

        Ok(Client {
            http,
            api_base_url,
            plar_version: self.plar_version,
            device_id: self.device_id,
            language: self.language,
        })
    }
}

impl Client {
    /// Creates a client configured for the production Physics-Lab API.
    pub fn new() -> Result<Self, Error> {
        Self::builder().build()
    }

    /// Starts building a client from the default configuration.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Returns the normalized API base URL.
    pub fn api_base_url(&self) -> &reqwest::Url {
        &self.api_base_url
    }

    /// Logs in anonymously and returns the session and current-user snapshot.
    ///
    /// The returned session borrows this client and therefore cannot outlive it.
    pub async fn anonymous_login(&self) -> Result<Session<'_>, Error> {
        let request = AuthenticateRequest {
            login: None,
            password: None,
            version: self.plar_version,
            device: Device {
                identifier: &self.device_id,
                language: &self.language,
            },
        };

        let response = self
            .http
            .post(
                self.api_base_url
                    .join("Users/Authenticate")
                    .expect("a validated base URL must join a static endpoint path"),
            )
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<AuthenticateResponse>()
            .await?;

        response.into_session(self)
    }
}

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

    fn authenticated_post(&self, path: &str) -> reqwest::RequestBuilder {
        self.client
            .http
            .post(
                self.client
                    .api_base_url
                    .join(path)
                    .expect("a validated base URL must join an endpoint path"),
            )
            .header("x-API-Token", self.token.as_deref().unwrap_or("null"))
            .header("x-API-AuthCode", &self.auth_code)
    }
}

/// A community content category understood by the Physics-Lab API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Category {
    Experiment,
    Discussion,
}

/// The user fields included in the authentication response.
#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: String,
    pub nickname: Option<String>,
    pub signature: Option<String>,
    pub is_bound: bool,
    pub gold: i64,
    pub level: i64,
    pub avatar: i64,
    pub avatar_region: i64,
    pub decoration: i64,
    pub verification: Value,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Physics-Lab API returned status {status}: {message}")]
    Api { status: i64, message: String },

    #[error("API response is missing required field `{0}`")]
    MissingField(&'static str),

    #[error("invalid API base URL `{value}`: {reason}")]
    InvalidBaseUrl { value: String, reason: String },
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct AuthenticateRequest<'a> {
    login: Option<&'a str>,
    password: Option<&'a str>,
    version: u32,
    device: Device<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct Device<'a> {
    identifier: &'a str,
    language: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct GetSummaryRequest<'a> {
    #[serde(rename = "ContentID")]
    content_id: &'a str,
    category: Category,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthenticateResponse {
    status: i64,
    message: Option<String>,
    token: Option<String>,
    auth_code: Option<String>,
    data: Option<AuthenticateData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AuthenticateData {
    user: WireUser,
    device_token: Option<String>,
    statistic: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WireUser {
    #[serde(rename = "ID")]
    id: String,
    nickname: Option<String>,
    signature: Option<String>,
    #[serde(rename = "IsBinded")]
    is_bound: bool,
    gold: i64,
    level: i64,
    avatar: i64,
    avatar_region: i64,
    decoration: i64,
    verification: Value,
}

impl AuthenticateResponse {
    fn into_session<'client>(self, client: &'client Client) -> Result<Session<'client>, Error> {
        if self.status != 200 {
            return Err(Error::Api {
                status: self.status,
                message: self
                    .message
                    .unwrap_or_else(|| "unknown server error".to_owned()),
            });
        }

        let auth_code = self.auth_code.ok_or(Error::MissingField("AuthCode"))?;
        let data = self.data.ok_or(Error::MissingField("Data"))?;

        Ok(Session {
            client,
            token: self.token,
            auth_code,
            device_token: data.device_token,
            current_user: CurrentUser {
                id: data.user.id,
                nickname: data.user.nickname,
                signature: data.user.signature,
                is_bound: data.user.is_bound,
                gold: data.user.gold,
                level: data.user.level,
                avatar: data.user.avatar,
                avatar_region: data.user.avatar_region,
                decoration: data.user.decoration,
                verification: data.user.verification,
            },
            statistic: data.statistic,
        })
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builder_normalizes_a_custom_base_url() {
        let client = Client::builder()
            .api_base_url("http://127.0.0.1:3000/api")
            .timeout(Duration::from_secs(1))
            .plar_version(2501)
            .device_id("test-device")
            .language("English")
            .build()
            .unwrap();

        assert_eq!(client.api_base_url().as_str(), "http://127.0.0.1:3000/api/");
        assert_eq!(client.plar_version, 2501);
        assert_eq!(client.device_id, "test-device");
        assert_eq!(client.language, "English");
    }

    #[test]
    fn builder_rejects_an_invalid_base_url() {
        let result = Client::builder().api_base_url("not a URL").build();

        assert!(matches!(result, Err(Error::InvalidBaseUrl { .. })));
    }

    #[test]
    fn anonymous_request_matches_the_python_client() {
        let request = AuthenticateRequest {
            login: None,
            password: None,
            version: 2411,
            device: Device {
                identifier: "7db01528cf13e2199e141c402d79190e",
                language: "Chinese",
            },
        };

        assert_eq!(
            serde_json::to_value(request).unwrap(),
            json!({
                "Login": null,
                "Password": null,
                "Version": 2411,
                "Device": {
                    "Identifier": "7db01528cf13e2199e141c402d79190e",
                    "Language": "Chinese"
                }
            })
        );
    }

    #[test]
    fn get_summary_request_matches_the_python_client() {
        let request = GetSummaryRequest {
            content_id: "summary-id",
            category: Category::Experiment,
        };

        assert_eq!(
            serde_json::to_value(request).unwrap(),
            json!({
                "ContentID": "summary-id",
                "Category": "Experiment"
            })
        );
    }

    #[test]
    fn api_status_is_checked() {
        assert!(check_api_response(&json!({ "Status": 200 })).is_ok());

        let error = check_api_response(&json!({
            "Status": 404,
            "Message": "not found"
        }))
        .unwrap_err();

        assert!(matches!(
            error,
            Error::Api {
                status: 404,
                message
            } if message == "not found"
        ));
    }

    #[test]
    fn successful_response_becomes_a_session() {
        let response: AuthenticateResponse = serde_json::from_value(json!({
            "Status": 200,
            "Message": "OK",
            "Token": null,
            "AuthCode": "auth-code",
            "Data": {
                "DeviceToken": "device-token",
                "Statistic": { "ActivityID": "example" },
                "User": {
                    "ID": "user-id",
                    "Nickname": "anonymous",
                    "Signature": null,
                    "IsBinded": false,
                    "Gold": 0,
                    "Level": 1,
                    "Avatar": 0,
                    "AvatarRegion": 0,
                    "Decoration": 0,
                    "Verification": null
                }
            }
        }))
        .unwrap();

        let client = Client::new().unwrap();
        let session = response.into_session(&client).unwrap();

        assert!(std::ptr::eq(session.client(), &client));
        assert_eq!(session.auth_code(), "auth-code");
        assert_eq!(session.current_user().id, "user-id");
        assert_eq!(
            session.current_user().nickname.as_deref(),
            Some("anonymous")
        );
    }
}
