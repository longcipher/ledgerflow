// LedgerFlow SDK error types

/// Unified error type for all SDK operations.
///
/// Designed to be convertible to language-specific exceptions in binding layers.
#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    /// HTTP-level error with status code and message.
    #[error("HTTP error: {status} - {message}")]
    Http { status: u16, message: String },

    /// Network / transport error.
    #[error("Network error: {0}")]
    Network(String),

    /// Failed to deserialize a response body.
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Caller supplied an invalid argument.
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

impl From<serde_json::Error> for SdkError {
    fn from(err: serde_json::Error) -> Self {
        SdkError::Deserialization(err.to_string())
    }
}

#[cfg(feature = "native")]
impl From<hpx::Error> for SdkError {
    fn from(err: hpx::Error) -> Self {
        SdkError::Network(err.to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_http() {
        let err = SdkError::Http {
            status: 404,
            message: "Not Found".into(),
        };
        assert_eq!(err.to_string(), "HTTP error: 404 - Not Found");
    }

    #[test]
    fn error_display_network() {
        let err = SdkError::Network("connection refused".into());
        assert_eq!(err.to_string(), "Network error: connection refused");
    }

    #[test]
    fn error_display_deserialization() {
        let err = SdkError::Deserialization("missing field `id`".into());
        assert_eq!(err.to_string(), "Deserialization error: missing field `id`");
    }

    #[test]
    fn error_display_invalid_input() {
        let err = SdkError::InvalidInput("amount must be positive".into());
        assert_eq!(err.to_string(), "Invalid input: amount must be positive");
    }

    #[test]
    fn error_from_serde_json() {
        let bad_json = "not json";
        let serde_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>(bad_json).expect_err("should fail");
        let sdk_err: SdkError = serde_err.into();
        match &sdk_err {
            SdkError::Deserialization(msg) => {
                assert!(
                    !msg.is_empty(),
                    "deserialization message should not be empty"
                );
            }
            other => panic!("expected Deserialization, got {other:?}"),
        }
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SdkError>();
    }
}
