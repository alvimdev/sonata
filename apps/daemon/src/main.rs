mod config;

use config::{ConfigError, ProviderKind, RuntimeConfig};
use sonata_core::{MediaProvider, PresencePublisher};
use sonata_provider_system::SystemProvider;
use sonata_rpc::DiscordPublisher;
use thiserror::Error;
use tracing::warn;

#[derive(Debug, Error)]
enum DaemonError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Provider(#[from] sonata_core::Error),
}

#[tokio::main]
async fn main() -> Result<(), DaemonError> {
    tracing_subscriber::fmt().with_target(false).init();
    let config = RuntimeConfig::load()?;
    run(config).await
}

async fn run(config: RuntimeConfig) -> Result<(), DaemonError> {
    let mut publisher = DiscordPublisher::connect(
        config
            .discord_client_id()
            .ok_or(ConfigError::MissingDiscordClientId)?,
    )
    .await?;

    match config.provider {
        ProviderKind::System => {
            let mut provider = SystemProvider::connect(config.system.player.clone()).await?;

            loop {
                match provider.next_event().await {
                    Ok(event) => {
                        if let Err(error) = publisher.publish(&event).await {
                            warn!("discord publish failed: {error}");
                        }
                    }
                    Err(error) => {
                        warn!("system provider failed: {error}");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        provider = SystemProvider::connect(config.system.player.clone()).await?;
                    }
                }
            }
        }
        ProviderKind::Browser => Err(ConfigError::BrowserProviderUnavailable.into()),
    }
}
