use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{MediaSource, PlaybackState, Track};

/// An event emitted by the single media provider selected by the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaEvent {
    /// The application or service to which this event belongs.
    pub source: MediaSource,
    /// The change observed in the media session.
    pub kind: MediaEventKind,
}

impl MediaEvent {
    pub fn track_changed(source: MediaSource, track: Track) -> Self {
        Self {
            source,
            kind: MediaEventKind::TrackChanged { track },
        }
    }

    pub fn playback_state_changed(source: MediaSource, state: PlaybackState) -> Self {
        Self {
            source,
            kind: MediaEventKind::PlaybackStateChanged { state },
        }
    }

    pub fn position_changed(source: MediaSource, position: Duration) -> Self {
        Self {
            source,
            kind: MediaEventKind::PositionChanged { position },
        }
    }

    pub fn session_ended(source: MediaSource) -> Self {
        Self {
            source,
            kind: MediaEventKind::SessionEnded,
        }
    }
}

/// The media-session changes that a provider can report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MediaEventKind {
    TrackChanged { track: Track },
    PlaybackStateChanged { state: PlaybackState },
    PositionChanged { position: Duration },
    SessionEnded,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{MediaSource, PlaybackState};

    use super::{MediaEvent, MediaEventKind};

    #[test]
    fn position_event_keeps_its_source_and_duration() {
        let source = MediaSource::browser("music.youtube.com").unwrap();
        let event = MediaEvent::position_changed(source.clone(), Duration::from_secs(42));

        assert_eq!(event.source, source);
        assert_eq!(
            event.kind,
            MediaEventKind::PositionChanged {
                position: Duration::from_secs(42)
            }
        );
    }

    #[test]
    fn state_changes_are_explicit() {
        let event = MediaEvent::playback_state_changed(
            MediaSource::system("Spotify").unwrap(),
            PlaybackState::Paused,
        );

        assert!(matches!(
            event.kind,
            MediaEventKind::PlaybackStateChanged {
                state: PlaybackState::Paused
            }
        ));
    }
}
