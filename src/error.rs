use thiserror::Error;

#[derive(Error, Debug)]
pub enum BBDDError {
    #[error("HTTP request error: {0}")]
    HttpRequestError(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("API error: code {code}, message: {message}")]
    ApiError { code: i32, message: String },

    #[error("Param error: {0}")]
    ParamError(String),

    #[error("State error: {0}")]
    StateError(String),
}

pub type BBDDResult<T> = std::result::Result<T, BBDDError>;

pub type Error = BBDDError;
pub type Result<T> = BBDDResult<T>;
