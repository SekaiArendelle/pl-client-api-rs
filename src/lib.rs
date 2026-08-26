use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const DEFAULT_BASE_URL: &str = "https://physics-api-cn.turtlesim.com";
const DEFAULT_PLAR_VERSION: u32 = 2411;
const DEFAULT_DEVICE_ID: &str = "7db01528cf13e2199e141c402d79190e";

/// An asynchronous client for the Physics-Lab API.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
}

impl Client {
    /// Creates a client configured for the production Physics-Lab API.
    pub fn new() -> Result<Self, Error> {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    /// Creates a client with a custom base URL.
    ///
    /// This is mainly useful for tests that run against a local mock server.
    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self, Error> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;

        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
        })
    }

    /// Logs in anonymously and returns the session and current-user snapshot.
    pub async fn anonymous_login(&self) -> Result<Session, Error> {
        let request = AuthenticateRequest {
            login: None,
            password: None,
            version: DEFAULT_PLAR_VERSION,
            device: Device {
                identifier: DEFAULT_DEVICE_ID,
                language: "Chinese",
            },
        };

        let response = self
            .http
            .post(format!("{}/Users/Authenticate", self.base_url))
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<AuthenticateResponse>()
            .await?;

        Session::try_from(response)
    }
}

/// Information returned by a successful login.
///
/// Credentials are intentionally private so they are not printed accidentally.
pub struct Session {
    token: Option<String>,
    auth_code: String,
    device_token: Option<String>,
    current_user: CurrentUser,
    statistic: Value,
}

impl Session {
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

    #[error("successful authentication response is missing `{0}`")]
    MissingField(&'static str),
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

impl TryFrom<AuthenticateResponse> for Session {
    type Error = Error;

    fn try_from(response: AuthenticateResponse) -> Result<Self, Self::Error> {
        if response.status != 200 {
            return Err(Error::Api {
                status: response.status,
                message: response
                    .message
                    .unwrap_or_else(|| "unknown server error".to_owned()),
            });
        }

        let auth_code = response.auth_code.ok_or(Error::MissingField("AuthCode"))?;
        let data = response.data.ok_or(Error::MissingField("Data"))?;

        Ok(Self {
            token: response.token,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn anonymous_request_matches_the_python_client() {
        let request = AuthenticateRequest {
            login: None,
            password: None,
            version: DEFAULT_PLAR_VERSION,
            device: Device {
                identifier: DEFAULT_DEVICE_ID,
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

        let session = Session::try_from(response).unwrap();

        assert_eq!(session.auth_code(), "auth-code");
        assert_eq!(session.current_user().id, "user-id");
        assert_eq!(
            session.current_user().nickname.as_deref(),
            Some("anonymous")
        );
    }
}
