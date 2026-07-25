use thiserror::Error;

/// Errors shared by Sonata's domain boundaries.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A provider could not produce its next event.
    #[error("media provider error: {0}")]
    Provider(String),

    /// A publisher could not apply a media event.
    #[error("presence publisher error: {0}")]
    Publisher(String),

    /// A domain model received invalid media metadata.
    #[error("invalid media data: {0}")]
    InvalidMediaData(String),

    /// An implementation does not support the requested operation.
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}

/// A Sonata domain result.
pub type Result<T> = std::result::Result<T, Error>;
