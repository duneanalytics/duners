use std::fmt;
use serde::Deserialize;

/// Error payload returned by the Dune API when a request fails (e.g. invalid API key, query not found).
#[derive(Deserialize, Debug)]
pub struct DuneError {
    /// Human-readable error message from Dune.
    pub error: String,
}

/// All errors that can occur when calling the Dune API or parsing responses.
///
/// Use `?` in async functions that return `Result<_, DuneRequestError>` to propagate errors.
/// Implements [`std::error::Error`] and [`Display`](fmt::Display) for logging and error reporting.
#[derive(Debug, PartialEq)]
pub enum DuneRequestError {
    /// Error returned by the Dune API. Common messages include:
    /// - `"invalid API Key"`
    /// - `"Query not found"`
    /// - `"The requested execution ID (ID: …) is invalid."`
    Dune(String),
    /// Network or HTTP errors from the underlying request (e.g. connection failed, timeout).
    Request(String),
}

impl fmt::Display for DuneRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DuneRequestError::Dune(msg) => write!(f, "Dune API error: {}", msg),
            DuneRequestError::Request(msg) => write!(f, "request error: {}", msg),
        }
    }
}

impl std::error::Error for DuneRequestError {}

impl From<DuneError> for DuneRequestError {
    fn from(value: DuneError) -> Self {
        DuneRequestError::Dune(value.error)
    }
}

impl From<reqwest::Error> for DuneRequestError {
    fn from(value: reqwest::Error) -> Self {
        DuneRequestError::Request(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn error_parsing() {
        let err = reqwest::get("invalid-url").await.unwrap_err();
        assert_eq!(
            DuneRequestError::from(err),
            DuneRequestError::Request("builder error".to_string())
        );
        assert_eq!(
            DuneRequestError::from(DuneError {
                error: "broken".to_string()
            }),
            DuneRequestError::Dune("broken".to_string())
        )
    }

    #[test]
    fn derive_debug() {
        assert_eq!(
            format!(
                "{:?}",
                DuneError {
                    error: "broken".to_string()
                }
            ),
            "DuneError { error: \"broken\" }"
        );
    }
}
