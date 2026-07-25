use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use sonata_core::{
    Error, MediaEvent, MediaSource, PlaybackState, Result, Track,
};
use tokio::sync::mpsc;
use windows::{
    core::Result as WindowsResult,
    Foundation::TypedEventHandler,
    Media::Control::{
        GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionMediaProperties,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    },
};

/// Windows implementation backed by Global System Media Transport Controls.
pub(super) struct PlatformProvider {
    events: mpsc::UnboundedReceiver<Result<MediaEvent>>,
    _manager: GlobalSystemMediaTransportControlsSessionManager,
    manager_token: i64,
    subscriptions: Arc<Mutex<Option<SessionSubscriptions>>>,
}

impl PlatformProvider {
    pub(super) async fn connect(_player: Option<String>) -> Result<Self> {
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .map_err(windows_error("could not request the Windows media session manager"))?
            .get()
            .map_err(windows_error("could not initialize the Windows media session manager"))?;
        let (sender, events) = mpsc::unbounded_channel();
        let subscriptions = Arc::new(Mutex::new(None));
        let handler_subscriptions = Arc::clone(&subscriptions);
        let handler_sender = sender.clone();
        let manager_token = manager
            .CurrentSessionChanged(&TypedEventHandler::new(move |manager, _| {
                let result = if let Some(manager) = manager {
                    replace_current_session(manager, &handler_subscriptions, &handler_sender)
                } else {
                    clear_session(&handler_subscriptions, Some(&handler_sender));
                    Ok(())
                };
                send_result(&handler_sender, result);
                Ok(())
            }))
            .map_err(windows_error("could not observe Windows media session changes"))?;

        replace_current_session(&manager, &subscriptions, &sender)?;

        Ok(Self {
            events,
            _manager: manager,
            manager_token,
            subscriptions,
        })
    }

    pub(super) async fn next_event(&mut self) -> Result<MediaEvent> {
        self.events.recv().await.unwrap_or_else(|| {
            Err(Error::Provider(
                "the Windows media event observer stopped unexpectedly".into(),
            ))
        })
    }
}

impl Drop for PlatformProvider {
    fn drop(&mut self) {
        let _ = self._manager.RemoveCurrentSessionChanged(self.manager_token);
        clear_session(&self.subscriptions);
    }
}

struct SessionSubscriptions {
    session: GlobalSystemMediaTransportControlsSession,
    source: MediaSource,
    media_properties_token: i64,
    playback_info_token: i64,
    timeline_token: i64,
}

impl SessionSubscriptions {
    fn new(
        session: GlobalSystemMediaTransportControlsSession,
        sender: &mpsc::UnboundedSender<Result<MediaEvent>>,
    ) -> Result<Self> {
        let source = source_for(&session)?;
        send_initial_events(&session, &source, sender);

        let media_sender = sender.clone();
        let media_properties_token = session
            .MediaPropertiesChanged(&TypedEventHandler::new(move |session, _| {
                if let Some(session) = session {
                    send_media_properties(session, &media_sender);
                }
                Ok(())
            }))
            .map_err(windows_error("could not observe Windows media properties"))?;

        let playback_sender = sender.clone();
        let playback_info_token = session
            .PlaybackInfoChanged(&TypedEventHandler::new(move |session, _| {
                if let Some(session) = session {
                    send_playback_state(session, &playback_sender);
                }
                Ok(())
            }))
            .map_err(windows_error("could not observe Windows playback state"))?;

        let timeline_sender = sender.clone();
        let timeline_token = session
            .TimelinePropertiesChanged(&TypedEventHandler::new(move |session, _| {
                if let Some(session) = session {
                    send_position(session, &timeline_sender);
                }
                Ok(())
            }))
            .map_err(windows_error("could not observe the Windows media timeline"))?;

        Ok(Self {
            session,
            source,
            media_properties_token,
            playback_info_token,
            timeline_token,
        })
    }

    fn unregister(self) {
        let _ = self
            .session
            .RemoveMediaPropertiesChanged(self.media_properties_token);
        let _ = self
            .session
            .RemovePlaybackInfoChanged(self.playback_info_token);
        let _ = self
            .session
            .RemoveTimelinePropertiesChanged(self.timeline_token);
    }
}

fn replace_current_session(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    subscriptions: &Arc<Mutex<Option<SessionSubscriptions>>>,
    sender: &mpsc::UnboundedSender<Result<MediaEvent>>,
) -> Result<()> {
    let session = match manager.GetCurrentSession() {
        Ok(session) => session,
        Err(_) => {
            clear_session(subscriptions, Some(sender));
            return Ok(());
        }
    };
    let next = SessionSubscriptions::new(session, sender)?;
    let previous = replace_session(subscriptions, next);
    if let Some(previous) = previous {
        previous.unregister();
    }

    Ok(())
}

fn send_initial_events(
    session: &GlobalSystemMediaTransportControlsSession,
    source: &MediaSource,
    sender: &mpsc::UnboundedSender<Result<MediaEvent>>,
) {
    send_result(sender, media_event(session, source));
    send_result(sender, playback_event(session, source));
    send_result(sender, position_event(session, source));
}

fn send_media_properties(
    session: &GlobalSystemMediaTransportControlsSession,
    sender: &mpsc::UnboundedSender<Result<MediaEvent>>,
) {
    let result = source_for(session).and_then(|source| media_event(session, &source));
    send_result(sender, result);
}

fn send_playback_state(
    session: &GlobalSystemMediaTransportControlsSession,
    sender: &mpsc::UnboundedSender<Result<MediaEvent>>,
) {
    let result = source_for(session).and_then(|source| playback_event(session, &source));
    send_result(sender, result);
}

fn send_position(
    session: &GlobalSystemMediaTransportControlsSession,
    sender: &mpsc::UnboundedSender<Result<MediaEvent>>,
) {
    let result = source_for(session).and_then(|source| position_event(session, &source));
    send_result(sender, result);
}

fn media_event(
    session: &GlobalSystemMediaTransportControlsSession,
    source: &MediaSource,
) -> Result<MediaEvent> {
    let properties = session
        .TryGetMediaPropertiesAsync()
        .map_err(windows_error("could not request Windows media properties"))?
        .get()
        .map_err(windows_error("could not read Windows media properties"))?;

    Ok(MediaEvent::track_changed(source.clone(), track_from(properties)?))
}

fn playback_event(
    session: &GlobalSystemMediaTransportControlsSession,
    source: &MediaSource,
) -> Result<MediaEvent> {
    let status = session
        .GetPlaybackInfo()
        .map_err(windows_error("could not read Windows playback information"))?
        .PlaybackStatus()
        .map_err(windows_error("could not read Windows playback status"))?;

    Ok(MediaEvent::playback_state_changed(
        source.clone(),
        playback_state(status)?,
    ))
}

fn position_event(
    session: &GlobalSystemMediaTransportControlsSession,
    source: &MediaSource,
) -> Result<MediaEvent> {
    let position = session
        .GetTimelineProperties()
        .map_err(windows_error("could not read the Windows media timeline"))?
        .Position()
        .map_err(windows_error("could not read the Windows media position"))?;

    Ok(MediaEvent::position_changed(
        source.clone(),
        duration_from_ticks(position.Duration)?,
    ))
}

fn source_for(session: &GlobalSystemMediaTransportControlsSession) -> Result<MediaSource> {
    let application = session
        .SourceAppUserModelId()
        .map_err(windows_error("could not identify the Windows media source"))?
        .to_string();

    MediaSource::system(application)
}

fn track_from(properties: GlobalSystemMediaTransportControlsSessionMediaProperties) -> Result<Track> {
    let mut track = Track::new(
        properties
            .Title()
            .map_err(windows_error("could not read the Windows media title"))?
            .to_string(),
    )?;
    let artist = properties
        .Artist()
        .map_err(windows_error("could not read the Windows media artist"))?
        .to_string();
    let album = properties
        .AlbumTitle()
        .map_err(windows_error("could not read the Windows media album"))?
        .to_string();

    if !artist.trim().is_empty() {
        track.artists.push(artist);
    }
    if !album.trim().is_empty() {
        track.album = Some(album);
    }

    Ok(track)
}

fn playback_state(status: GlobalSystemMediaTransportControlsSessionPlaybackStatus) -> Result<PlaybackState> {
    match status {
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => Ok(PlaybackState::Playing),
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused => Ok(PlaybackState::Paused),
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped
        | GlobalSystemMediaTransportControlsSessionPlaybackStatus::Closed
        | GlobalSystemMediaTransportControlsSessionPlaybackStatus::Opened
        | GlobalSystemMediaTransportControlsSessionPlaybackStatus::Changing => Ok(PlaybackState::Stopped),
        status => Err(Error::Provider(format!(
            "received unknown Windows playback status: {}",
            status.0
        ))),
    }
}

fn duration_from_ticks(ticks: i64) -> Result<Duration> {
    let ticks = u64::try_from(ticks)
        .map_err(|_| Error::Provider("received a negative Windows media time".into()))?;
    let nanos = ticks
        .checked_mul(100)
        .ok_or_else(|| Error::Provider("Windows media time is too large".into()))?;

    Ok(Duration::from_nanos(nanos))
}

fn send_result(sender: &mpsc::UnboundedSender<Result<MediaEvent>>, result: Result<MediaEvent>) {
    let _ = sender.send(result);
}

fn replace_session(
    subscriptions: &Arc<Mutex<Option<SessionSubscriptions>>>,
    next: SessionSubscriptions,
) -> Option<SessionSubscriptions> {
    subscriptions.lock().expect("Windows session mutex poisoned").replace(next)
}

fn take_session(
    subscriptions: &Arc<Mutex<Option<SessionSubscriptions>>>,
) -> Option<SessionSubscriptions> {
    subscriptions.lock().expect("Windows session mutex poisoned").take()
}

fn clear_session(subscriptions: &Arc<Mutex<Option<SessionSubscriptions>>>) {
    clear_session_with_sender(subscriptions, None);
}

fn clear_session_with_sender(
    subscriptions: &Arc<Mutex<Option<SessionSubscriptions>>>,
    sender: Option<&mpsc::UnboundedSender<Result<MediaEvent>>>,
) {
    if let Some(session) = take_session(subscriptions) {
        if let Some(sender) = sender {
            let _ = sender.send(Ok(MediaEvent::session_ended(session.source.clone())));
        }
        session.unregister();
    }
}

fn windows_error(context: &'static str) -> impl FnOnce(windows::core::Error) -> Error {
    move |error| Error::Provider(format!("{context}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_windows_ticks_to_a_duration() {
        assert_eq!(duration_from_ticks(15_000_000).unwrap(), Duration::from_millis(1500));
    }
}
