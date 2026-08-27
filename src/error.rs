use thiserror::Error;

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
