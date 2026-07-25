use async_trait::async_trait;

use crate::{MediaEvent, Result};

/// A destination that reflects media events in an external presence system.
#[async_trait]
pub trait PresencePublisher: Send {
    /// Applies one media event to the external presence system.
    async fn publish(&mut self, event: &MediaEvent) -> Result<()>;

    /// Clears any currently published presence.
    async fn clear(&mut self) -> Result<()>;

    /// Shuts the publisher down and releases any external connection.
    async fn shutdown(&mut self) -> Result<()>;
}
