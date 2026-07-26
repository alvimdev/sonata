use discord_rich_presence::{activity::Activity, DiscordIpc, DiscordIpcClient};
use tracing::{debug, info, warn};

use super::{Error, Result};

/// IPC wrapper that keeps the Discord connection encapsulated.
pub struct DiscordClient {
    client_id: String,
    ipc: Option<DiscordIpcClient>,
    last_activity: Option<Activity<'static>>,
}

impl DiscordClient {
    pub fn new(client_id: impl Into<String>) -> Result<Self> {
        let client_id = client_id.into();
        if client_id.trim().is_empty() {
            return Err(Error::MissingClientId);
        }

        Ok(Self {
            client_id,
            ipc: None,
            last_activity: None,
        })
    }

    pub async fn connect(&mut self) -> Result<()> {
        self.connect_ipc()
    }

    pub async fn set_activity(&mut self, activity: Activity<'static>) -> Result<()> {
        self.last_activity = Some(activity.clone());
        self.send_activity(activity)
    }

    pub async fn clear_activity(&mut self) -> Result<()> {
        self.last_activity = None;
        if let Some(ipc) = self.ipc.as_mut() {
            if let Err(error) = ipc.clear_activity() {
                warn!(target: "sonata::discord", error = %error, "IPC Error");
                self.ipc = None;
                return Err(error.into());
            }
            info!(target: "sonata::discord", "Activity Cleared");
        }

        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(mut ipc) = self.ipc.take() {
            if let Err(error) = ipc.close() {
                warn!(target: "sonata::discord", error = %error, "IPC Error");
                return Err(error.into());
            }
            info!(target: "sonata::discord", "Disconnected");
        }

        Ok(())
    }

    fn connect_ipc(&mut self) -> Result<()> {
        if self.ipc.is_some() {
            return Ok(());
        }

        info!(target: "sonata::discord", "Reconnecting");
        let mut ipc = DiscordIpcClient::new(&self.client_id)?;
        ipc.connect()?;
        debug!(target: "sonata::discord", "Connected");
        self.ipc = Some(ipc);

        Ok(())
    }

    fn send_activity(&mut self, activity: Activity<'static>) -> Result<()> {
        self.connect_ipc()?;

        let result = self
            .ipc
            .as_mut()
            .ok_or(Error::MissingClientId)?
            .set_activity(activity);

        match result {
            Ok(()) => {
                info!(target: "sonata::discord", "Activity Updated");
                Ok(())
            }
            Err(error) => {
                warn!(target: "sonata::discord", error = %error, "IPC Error");
                self.ipc = None;
                self.reconnect_and_restore()
            }
        }
    }

    fn reconnect_and_restore(&mut self) -> Result<()> {
        self.connect_ipc()?;

        if let Some(activity) = self.last_activity.clone() {
            let result = self
                .ipc
                .as_mut()
                .ok_or(Error::MissingClientId)?
                .set_activity(activity);

            if let Err(error) = result {
                self.ipc = None;
                return Err(error.into());
            }

            info!(target: "sonata::discord", "Activity Updated");
        }

        Ok(())
    }
}