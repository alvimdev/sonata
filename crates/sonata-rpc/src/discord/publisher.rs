use std::time::Duration;

use async_trait::async_trait;
use sonata_core::{MediaEvent, MediaEventKind, PlaybackState, PresencePublisher, Result, Track};

use super::{activity::build_activity, DiscordClient};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct PlaybackSnapshot {
    track: Option<Track>,
    playback_state: Option<PlaybackState>,
    position: Option<Duration>,
}

/// Discord Rich Presence publisher.
pub struct DiscordPublisher {
    client: DiscordClient,
    snapshot: PlaybackSnapshot,
    last_rendered: Option<PlaybackSnapshot>,
}

impl DiscordPublisher {
    pub async fn connect(client_id: impl Into<String>) -> Result<Self> {
        let mut client = DiscordClient::new(client_id.into()).map_err(map_error)?;
        client.connect().await.map_err(map_error)?;

        Ok(Self {
            client,
            snapshot: PlaybackSnapshot::default(),
            last_rendered: None,
        })
    }

    fn update_snapshot(&mut self, event: &MediaEvent) {
        match &event.kind {
            MediaEventKind::TrackChanged { track } => self.snapshot.track = Some(track.clone()),
            MediaEventKind::PlaybackStateChanged { state } => {
                self.snapshot.playback_state = Some(*state)
            }
            MediaEventKind::PositionChanged { position } => self.snapshot.position = Some(*position),
            MediaEventKind::SessionEnded => self.snapshot = PlaybackSnapshot::default(),
        }
    }

    fn should_clear(&self) -> bool {
        matches!(self.snapshot.playback_state, Some(PlaybackState::Stopped))
            || self.snapshot.track.is_none()
    }

    fn should_render(&self) -> bool {
        match &self.last_rendered {
            None => true,
            Some(last) => last.track != self.snapshot.track || last.playback_state != self.snapshot.playback_state,
        }
    }

    async fn publish_snapshot(&mut self) -> Result<()> {
        if self.should_clear() {
            return self.clear().await;
        }

        if !self.should_render() {
            return Ok(());
        }

        let track = match &self.snapshot.track {
            Some(track) => track,
            None => return Ok(()),
        };

        let state = self
            .snapshot
            .playback_state
            .unwrap_or(PlaybackState::Playing);
        let activity = match build_activity(track, state, self.snapshot.position) {
            Some(activity) => activity,
            None => return Ok(()),
        };

        self.client.set_activity(activity).await.map_err(map_error)?;
        self.last_rendered = Some(self.snapshot.clone());

        Ok(())
    }
}

#[async_trait]
impl PresencePublisher for DiscordPublisher {
    async fn publish(&mut self, event: &MediaEvent) -> Result<()> {
        self.update_snapshot(event);

        if matches!(&event.kind, MediaEventKind::SessionEnded) {
            self.clear().await
        } else {
            self.publish_snapshot().await
        }
    }

    async fn clear(&mut self) -> Result<()> {
        if self.last_rendered.is_none() {
            self.snapshot = PlaybackSnapshot::default();
            return Ok(());
        }

        self.client.clear_activity().await.map_err(map_error)?;
        self.snapshot = PlaybackSnapshot::default();
        self.last_rendered = None;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.client.shutdown().await.map_err(map_error)
    }
}

fn map_error(error: super::Error) -> sonata_core::Error {
    sonata_core::Error::Publisher(error.to_string())
}