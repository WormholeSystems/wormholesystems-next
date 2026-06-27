use thiserror::Error;

#[derive(Debug, Error)]
pub enum EsiError {
    #[error("ESI request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("ESI returned {status}: {body}")]
    Api { status: u16, body: String },
    #[error("SSO error: {0}")]
    Auth(String),
    #[error("token validation failed: {0}")]
    Jwt(String),
    #[error("missing required scope: {0}")]
    MissingScope(String),
}

pub type Result<T> = std::result::Result<T, EsiError>;
