use thiserror::Error;

/// Errors produced by the Discord RPC layer.
#[derive(Debug, Error)]
pub enum Error {
    #[error("discord client id is missing")]
    MissingClientId,

    #[error("discord IPC error: {0}")]
    Ipc(#[from] discord_rich_presence::error::Error),
}

/// A Discord RPC result.
pub type Result<T> = std::result::Result<T, Error>;