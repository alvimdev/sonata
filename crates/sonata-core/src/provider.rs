use async_trait::async_trait;

use crate::{MediaEvent, Result};

/// A source of ordered media-session events.
///
/// Implementations should await until an event is available instead of polling
/// their underlying platform API from the daemon's main loop.
#[async_trait]
pub trait MediaProvider: Send {
    /// Waits for and returns the next event emitted by this provider.
    async fn next_event(&mut self) -> Result<MediaEvent>;
}
