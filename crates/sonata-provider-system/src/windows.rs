//! Windows implementation backed by the Global System Media Transport Controls
//! (GSMTC). Entirely event-driven: no polling, no background loop. WinRT
//! notifies us through callbacks, which only enqueue a cheap signal; the
//! actual (blocking) reads happen sequentially in a per-session worker task
//! running on the blocking thread pool, so neither the Tokio runtime threads
//! nor the WinRT callback threads are ever blocked.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use sonata_core::{Error, MediaEvent, MediaSource, PlaybackState, Result, Track};
use tokio::{runtime::Handle, sync::mpsc, task::JoinHandle};
use tracing::{debug, warn};
use windows::{
    Foundation::TypedEventHandler,
    Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as SessionManager,
        GlobalSystemMediaTransportControlsSessionMediaProperties as MediaProperties,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
        GlobalSystemMediaTransportControlsSessionTimelineProperties as TimelineProperties,
    },
};

type Sender = mpsc::UnboundedSender<Result<MediaEvent>>;

/// Connects to GSMTC and exposes the resulting events through a channel.
///
/// This is the only public surface of the module: everything else is an
/// implementation detail of how the events get produced.
pub(super) struct PlatformProvider {
    events: mpsc::UnboundedReceiver<Result<MediaEvent>>,
    observer: Arc<Mutex<SessionObserver>>,
}

impl PlatformProvider {
    pub(super) async fn connect(_player: Option<String>) -> Result<Self> {
        let manager = SessionManager::RequestAsync()
            .map_err(windows_error("could not request the Windows media session manager"))?
            .await
            .map_err(windows_error("could not initialize the Windows media session manager"))?;
        debug!("connected");

        let (sender, events) = mpsc::unbounded_channel();
        let runtime = Handle::current();
        let observer = SessionObserver::start(manager, sender, runtime)?;

        Ok(Self { events, observer })
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
        if let Ok(mut observer) = self.observer.lock() {
            observer.shutdown();
        }
        debug!("provider shutdown");
    }
}

/// Owns the GSMTC session manager and tracks whichever session is currently
/// "active", re-subscribing every time the active session changes.
///
/// Wrapped in a single `Arc<Mutex<_>>` so that the `CurrentSessionChanged`
/// callback (which fires from WinRT with no access to `&mut self`) can reach
/// back in and replace the active subscription.
struct SessionObserver {
    manager: SessionManager,
    manager_token: i64,
    current_session: Option<SessionSubscriptions>,
    sender: Sender,
    runtime: Handle,
}

impl SessionObserver {
    fn start(manager: SessionManager, sender: Sender, runtime: Handle) -> Result<Arc<Mutex<Self>>> {
        let observer = Arc::new(Mutex::new(Self {
            manager: manager.clone(),
            manager_token: 0,
            current_session: None,
            sender,
            runtime,
        }));

        let callback_observer = Arc::clone(&observer);
        let manager_token = manager
            .CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
                on_current_session_changed(&callback_observer);
                Ok(())
            }))
            .map_err(windows_error("could not observe Windows media session changes"))?;
        debug!(token = manager_token, "callback registered: current session changed");

        let mut guard = observer.lock().expect("Windows session observer mutex poisoned");
        guard.manager_token = manager_token;
        guard.replace_session();
        drop(guard);

        Ok(observer)
    }

    /// Drops whatever session is currently subscribed to, requests the
    /// current one from the manager, and (if there is one) subscribes to it
    /// and emits an initial snapshot so the daemon has the current state
    /// immediately, without waiting for the next change notification.
    fn replace_session(&mut self) {
        let previous = self.current_session.take();

        let session = match self.manager.GetCurrentSession() {
            Ok(session) => session,
            Err(_) => {
                debug!("current session changed: none");
                if let Some(previous) = previous {
                    let source = previous.source.clone();
                    previous.unregister();
                    self.emit(Ok(MediaEvent::session_ended(source)));
                }
                return;
            }
        };

        if let Some(previous) = previous {
            previous.unregister();
        }

        debug!("current session changed");
        match SessionSubscriptions::new(session, self.sender.clone(), self.runtime.clone()) {
            Ok(subscriptions) => self.current_session = Some(subscriptions),
            Err(error) => {
                warn!(%error, "could not subscribe to the new Windows media session");
                self.emit(Err(error));
            }
        }
    }

    fn emit(&self, result: Result<MediaEvent>) {
        emit(&self.sender, result);
    }

    fn shutdown(&mut self) {
        let _ = self.manager.RemoveCurrentSessionChanged(self.manager_token);
        debug!("callback removed: current session changed");
        if let Some(session) = self.current_session.take() {
            session.unregister();
        }
    }
}

/// Called from the `CurrentSessionChanged` callback. Kept as a free function,
/// rather than inlined in the closure, so the locking and error handling read
/// like ordinary control flow.
fn on_current_session_changed(observer: &Arc<Mutex<SessionObserver>>) {
    match observer.lock() {
        Ok(mut observer) => observer.replace_session(),
        Err(_) => warn!("Windows session observer mutex poisoned"),
    }
}

/// What changed in the session and needs to be re-read and re-emitted.
/// Kept intentionally cheap: WinRT callbacks only ever construct one of
/// these and push it onto the refresh queue, they never do the actual read.
#[derive(Debug, Clone, Copy)]
enum Refresh {
    MediaProperties,
    PlaybackInfo,
    Timeline,
}

/// The callbacks registered against a single GSMTC session, plus the worker
/// task that turns `Refresh` signals into `MediaEvent`s, and what is needed
/// to unregister and shut everything down.
struct SessionSubscriptions {
    session: Session,
    source: MediaSource,
    media_properties_token: i64,
    playback_info_token: i64,
    timeline_token: i64,
    worker: JoinHandle<()>,
}

impl SessionSubscriptions {
    fn new(session: Session, sender: Sender, runtime: Handle) -> Result<Self> {
        let source = source_from(&session)?;

        let (refresh_tx, mut refresh_rx) = mpsc::unbounded_channel::<Refresh>();

        // One worker per active session. Reads signals off the queue in
        // order and performs the (blocking) WinRT read on the blocking
        // thread pool, one at a time, so events for a given session are
        // never reordered relative to each other, and neither a Tokio
        // worker thread nor a WinRT callback thread ever blocks.
        let worker = {
            let session = session.clone();
            let source = source.clone();
            let sender = sender.clone();
            runtime.spawn(async move {
                while let Some(kind) = refresh_rx.recv().await {
                    let session = session.clone();
                    let source = source.clone();
                    let result = tokio::task::spawn_blocking(move || match kind {
                        Refresh::MediaProperties => media_event(&session, &source),
                        Refresh::PlaybackInfo => playback_event(&session, &source),
                        Refresh::Timeline => position_event(&session, &source),
                    })
                    .await
                    .unwrap_or_else(|join_error| {
                        Err(Error::Provider(format!(
                            "a Windows media refresh task panicked: {join_error}"
                        )))
                    });

                    emit(&sender, result);
                }
            })
        };

        // Initial snapshot, so the daemon has the current state immediately
        // instead of waiting for the next WinRT notification.
        let _ = refresh_tx.send(Refresh::MediaProperties);
        let _ = refresh_tx.send(Refresh::PlaybackInfo);
        let _ = refresh_tx.send(Refresh::Timeline);

        let media_properties_token = {
            let refresh_tx = refresh_tx.clone();
            session
                .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    let _ = refresh_tx.send(Refresh::MediaProperties);
                    Ok(())
                }))
                .map_err(windows_error("could not observe Windows media properties"))?
        };
        debug!(token = media_properties_token, "callback registered: media properties changed");

        let playback_info_token = {
            let refresh_tx = refresh_tx.clone();
            session
                .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                    let _ = refresh_tx.send(Refresh::PlaybackInfo);
                    Ok(())
                }))
                .map_err(windows_error("could not observe Windows playback state"))?
        };
        debug!(token = playback_info_token, "callback registered: playback changed");

        let timeline_token = {
            let refresh_tx = refresh_tx.clone();
            session
                .TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    let _ = refresh_tx.send(Refresh::Timeline);
                    Ok(())
                }))
                .map_err(windows_error("could not observe the Windows media timeline"))?
        };
        debug!(token = timeline_token, "callback registered: timeline changed");

        Ok(Self {
            session,
            source,
            media_properties_token,
            playback_info_token,
            timeline_token,
            worker,
        })
    }

    fn unregister(self) {
        let _ = self.session.RemoveMediaPropertiesChanged(self.media_properties_token);
        let _ = self.session.RemovePlaybackInfoChanged(self.playback_info_token);
        let _ = self.session.RemoveTimelinePropertiesChanged(self.timeline_token);
        self.worker.abort();
        debug!("callback removed: session subscriptions");
    }
}

// ---------------------------------------------------------------------------
// Event construction
//
// Each of these reads one piece of session state and converts it into a
// `MediaEvent`. They are called both for the initial snapshot and from
// inside the per-session worker above, always on the blocking thread pool,
// so no conversion logic lives in the WinRT callbacks themselves.
// ---------------------------------------------------------------------------

fn media_event(session: &Session, source: &MediaSource) -> Result<MediaEvent> {
    let properties = session
        .TryGetMediaPropertiesAsync()
        .map_err(windows_error("could not request Windows media properties"))?
        .join()
        .map_err(windows_error("could not read Windows media properties"))?;

    Ok(MediaEvent::track_changed(
        source.clone(),
        track_from_properties(properties)?,
    ))
}

fn playback_event(session: &Session, source: &MediaSource) -> Result<MediaEvent> {
    let status = session
        .GetPlaybackInfo()
        .map_err(windows_error("could not read Windows playback information"))?
        .PlaybackStatus()
        .map_err(windows_error("could not read Windows playback status"))?;

    Ok(MediaEvent::playback_state_changed(
        source.clone(),
        playback_state_from_gsmtc(status)?,
    ))
}

fn position_event(session: &Session, source: &MediaSource) -> Result<MediaEvent> {
    let timeline = session
        .GetTimelineProperties()
        .map_err(windows_error("could not read the Windows media timeline"))?;

    Ok(MediaEvent::position_changed(
        source.clone(),
        timeline_to_duration(timeline)?,
    ))
}

fn source_from(session: &Session) -> Result<MediaSource> {
    let application = session
        .SourceAppUserModelId()
        .map_err(windows_error("could not identify the Windows media source"))?
        .to_string();

    MediaSource::system(application)
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

fn track_from_properties(properties: MediaProperties) -> Result<Track> {
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

fn playback_state_from_gsmtc(status: PlaybackStatus) -> Result<PlaybackState> {
    match status {
        PlaybackStatus::Playing => Ok(PlaybackState::Playing),
        PlaybackStatus::Paused => Ok(PlaybackState::Paused),
        PlaybackStatus::Stopped
        | PlaybackStatus::Closed
        | PlaybackStatus::Opened
        | PlaybackStatus::Changing => Ok(PlaybackState::Stopped),
        status => Err(Error::Provider(format!(
            "received unknown Windows playback status: {}",
            status.0
        ))),
    }
}

fn timeline_to_duration(timeline: TimelineProperties) -> Result<Duration> {
    let position = timeline
        .Position()
        .map_err(windows_error("could not read the Windows media position"))?;

    duration_from_ticks(position.Duration)
}

fn duration_from_ticks(ticks: i64) -> Result<Duration> {
    let ticks = u64::try_from(ticks)
        .map_err(|_| Error::Provider("received a negative Windows media time".into()))?;
    let nanos = ticks
        .checked_mul(100)
        .ok_or_else(|| Error::Provider("Windows media time is too large".into()))?;

    Ok(Duration::from_nanos(nanos))
}

// ---------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------

fn emit(sender: &Sender, result: Result<MediaEvent>) {
    let _ = sender.send(result);
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

    #[test]
    fn rejects_negative_windows_ticks() {
        assert!(duration_from_ticks(-1).is_err());
    }
}