//! Native operating-system media-session provider.

#![forbid(unsafe_code)]

use async_trait::async_trait;
use sonata_core::{MediaEvent, MediaProvider, Result};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod unsupported;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux::PlatformProvider;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
use unsupported::PlatformProvider;
#[cfg(target_os = "windows")]
use windows::PlatformProvider;

/// Observes one media session through the native API of the current platform.
pub struct SystemProvider {
    inner: PlatformProvider,
}

impl SystemProvider {
    /// Connects to a platform media session.
    ///
    /// On Linux, `player` is the MPRIS D-Bus service name, such as
    /// `org.mpris.MediaPlayer2.<player>`. On Windows, the active system media
    /// session is used and this value is ignored.
    pub async fn connect(player: Option<String>) -> Result<Self> {
        Ok(Self {
            inner: PlatformProvider::connect(player).await?,
        })
    }
}

#[async_trait]
impl MediaProvider for SystemProvider {
    async fn next_event(&mut self) -> Result<MediaEvent> {
        self.inner.next_event().await
    }
}

#[cfg(test)]
mod tests {
    use sonata_core::MediaProvider;

    use super::SystemProvider;

    #[test]
    fn system_provider_implements_the_core_contract() {
        fn assert_media_provider<T: MediaProvider>() {}

        assert_media_provider::<SystemProvider>();
    }
}
