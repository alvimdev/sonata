//! Wire format trocado com a extensão via Native Messaging. Mantido separado
//! do domínio de propósito: o formato JSON aqui é decidido pela extensão
//! (camelCase, tipos "soltos"), e a conversão pra `sonata_core` normaliza
//! tudo pro contrato interno do projeto.

use std::time::Duration;

use serde::Deserialize;
use sonata_core::{Error, MediaEvent, MediaSource, PlaybackState, Result, Track};

#[derive(Debug, Deserialize)]
pub struct WireMessage {
    pub source: WireSource,
    pub kind: WireEventKind,
}

#[derive(Debug, Deserialize)]
pub struct WireSource {
    #[allow(dead_code)] // sempre "browser" hoje; mantido pra compatibilidade futura
    pub kind: String,
    pub application: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum WireEventKind {
    #[serde(rename = "track_changed")]
    TrackChanged { track: WireTrack },
    #[serde(rename = "playback_state_changed")]
    PlaybackStateChanged { state: WirePlaybackState },
    #[serde(rename = "position_changed")]
    PositionChanged {
        #[serde(rename = "positionMs")]
        position_ms: f64,
    },
    #[serde(rename = "session_ended")]
    SessionEnded,
}

#[derive(Debug, Deserialize)]
pub struct WireTrack {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    #[serde(rename = "durationMs")]
    pub duration_ms: Option<f64>,
    #[serde(rename = "artworkUrl")]
    pub artwork_url: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WirePlaybackState {
    Playing,
    Paused,
    Stopped,
}

impl WireMessage {
    pub fn into_media_event(self) -> Result<MediaEvent> {
        let source = MediaSource::browser(self.source.application)?;

        Ok(match self.kind {
            WireEventKind::TrackChanged { track } => {
                MediaEvent::track_changed(source, track.into_track()?)
            }
            WireEventKind::PlaybackStateChanged { state } => {
                MediaEvent::playback_state_changed(source, state.into())
            }
            WireEventKind::PositionChanged { position_ms } => {
                MediaEvent::position_changed(source, duration_from_millis(position_ms)?)
            }
            WireEventKind::SessionEnded => MediaEvent::session_ended(source),
        })
    }
}

impl WireTrack {
    fn into_track(self) -> Result<Track> {
        let mut track = Track::new(self.title)?;
        track.artists = self.artists;
        track.album = self.album;
        track.duration = self.duration_ms.map(duration_from_millis).transpose()?;
        track.artwork_url = self.artwork_url;
        track.url = self.url;
        Ok(track)
    }
}

impl From<WirePlaybackState> for PlaybackState {
    fn from(state: WirePlaybackState) -> Self {
        match state {
            WirePlaybackState::Playing => PlaybackState::Playing,
            WirePlaybackState::Paused => PlaybackState::Paused,
            WirePlaybackState::Stopped => PlaybackState::Stopped,
        }
    }
}

fn duration_from_millis(millis: f64) -> Result<Duration> {
    if !millis.is_finite() || millis < 0.0 {
        return Err(Error::InvalidMediaData(
            "received a negative or non-finite duration from the browser".into(),
        ));
    }
    Ok(Duration::from_secs_f64(millis / 1000.0))
}