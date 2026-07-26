mod config;

use config::{ConfigError, ProviderKind, RuntimeConfig};
use sonata_core::{MediaProvider, PresencePublisher};
use sonata_provider_system::SystemProvider;
use sonata_provider_browser::BrowserProvider;
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

use std::{env, fs::OpenOptions, path::PathBuf};

fn init_tracing() {
    let log_path = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("sonata.log")))
        .unwrap_or_else(|| PathBuf::from("sonata.log"));

    if let Ok(file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        tracing_subscriber::fmt()
            .with_writer(file)
            .with_target(false)
            .with_ansi(false)
            .init();
    }
}

#[tokio::main]
async fn main() -> Result<(), DaemonError> {
    init_tracing();
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
        ProviderKind::Browser => {
            let mut provider = BrowserProvider::connect()?;

            loop {
                match provider.next_event().await {
                    Ok(event) => {
                        if let Err(error) = publisher.publish(&event).await {
                            warn!("discord publish failed: {error}");
                        }
                    }
                    Err(error) => {
                        // Sem reconexão aqui de propósito: o processo é
                        // spawnado pelo browser via connectNative. Quando o
                        // canal fecha, o certo é o daemon encerrar — o
                        // browser sobe uma instância nova na próxima vez
                        // que precisar.
                        warn!("browser native messaging channel ended: {error}");
                        publisher.clear().await.ok();
                        break;
                    }
                }
            }

            publisher.shutdown().await.ok();
            Ok(())
        }
    }
}
