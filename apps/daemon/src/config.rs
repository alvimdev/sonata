use std::{env, fs, path::PathBuf};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Browser,
    System,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SystemConfig {
    pub player: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct DiscordConfig {
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RuntimeConfig {
    pub provider: ProviderKind,

    #[serde(default)]
    pub system: SystemConfig,

    #[serde(default)]
    pub discord: DiscordConfig,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read runtime config at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not parse runtime config at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("the selected provider requires system.player to be set on Linux")]
    MissingSystemPlayer,

    // #[error("browser provider is not implemented yet")]
    // BrowserProviderUnavailable,

    #[error("discord client id is missing; set discord.client_id or SONATA_DISCORD_CLIENT_ID")]
    MissingDiscordClientId,
}

impl RuntimeConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let path = config_path();
        let contents = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
            path: path.clone(),
            source,
        })?;
        let config: Self = toml::from_str(&contents).map_err(|source| ConfigError::Parse {
            path,
            source,
        })?;

        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if matches!(self.provider, ProviderKind::System)
            && cfg!(target_os = "linux")
            && self
                .system
                .player
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        {
            return Err(ConfigError::MissingSystemPlayer);
        }

        if self.discord_client_id().as_deref().map(str::trim).unwrap_or("").is_empty() {
            return Err(ConfigError::MissingDiscordClientId);
        }

        Ok(())
    }

    pub fn discord_client_id(&self) -> Option<String> {
        self.discord
            .client_id
            .clone()
            .or_else(|| env::var("SONATA_DISCORD_CLIENT_ID").ok())
    }
}

fn config_path() -> PathBuf {
    if let Some(path) = env::var_os("SONATA_CONFIG") {
        return PathBuf::from(path);
    }

    // O cwd não é confiável quando o daemon é spawnado pelo browser (native
    // messaging) — resolve relativo ao binário, não a quem chamou ele.
    env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("sonata.toml")))
        .unwrap_or_else(|| PathBuf::from("sonata.toml"))
}

#[cfg(test)]
mod tests {
    use super::{ProviderKind, RuntimeConfig};

    #[test]
    fn parses_the_system_provider_config() {
        let config: RuntimeConfig = toml::from_str(
            r#"
provider = "system"

[system]
player = "org.mpris.MediaPlayer2.<player>"
"#,
        )
        .unwrap();

        assert!(matches!(config.provider, ProviderKind::System));
        assert_eq!(
            config.system.player.as_deref(),
            Some("org.mpris.MediaPlayer2.<player>")
        );
    }
}