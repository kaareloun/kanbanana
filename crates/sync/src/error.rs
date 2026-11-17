#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("{0}")]
    Store(#[from] store::Error),
    #[error("{0}")]
    Api(#[from] linear_api::Error),
    #[error("no API key for workspace {0}")]
    NoKey(String),
    #[error("Linear rate limit, retry after {}", .0.format("%H:%M:%S UTC"))]
    Gated(chrono::DateTime<chrono::Utc>),
    #[error("local tasks cannot have comments yet")]
    LocalComment,
    #[error("issue {0} is not in the cache")]
    NoIssue(String),
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Retry delay when Linear rate limited the call.
    pub(crate) fn retry_after_secs(&self) -> Option<u64> {
        match self {
            Error::Api(linear_api::Error::RateLimited { retry_after_secs }) => {
                Some(*retry_after_secs)
            }
            _ => None,
        }
    }
}
