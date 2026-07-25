use sonata_core::{Error, MediaEvent, Result};

pub(super) struct PlatformProvider;

impl PlatformProvider {
    pub(super) async fn connect(_player: String) -> Result<Self> {
        Err(Error::Unsupported(
            "the system provider is only available on Linux and Windows in v1".into(),
        ))
    }

    pub(super) async fn next_event(&mut self) -> Result<MediaEvent> {
        Err(Error::Unsupported(
            "the system provider is only available on Linux and Windows in v1".into(),
        ))
    }
}
