use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Identifies the kind of integration that observed a media session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaSourceKind {
    Browser,
    System,
    /// A future or third-party provider kind.
    Custom(String),
}

/// Identifies the application or service associated with a media session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaSource {
    pub kind: MediaSourceKind,
    pub application: String,
}

impl MediaSource {
    pub fn new(kind: MediaSourceKind, application: impl Into<String>) -> Result<Self> {
        let application = application.into();
        if application.trim().is_empty() {
            return Err(Error::InvalidMediaData(
                "media source application cannot be empty".into(),
            ));
        }

        Ok(Self { kind, application })
    }

    pub fn browser(application: impl Into<String>) -> Result<Self> {
        Self::new(MediaSourceKind::Browser, application)
    }

    pub fn system(application: impl Into<String>) -> Result<Self> {
        Self::new(MediaSourceKind::System, application)
    }
}

/// The current state of a media session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
}

/// Metadata that publishers can expose for the current item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,
    pub artwork_url: Option<String>,
    pub url: Option<String>,
}

impl Track {
    pub fn new(title: impl Into<String>) -> Result<Self> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(Error::InvalidMediaData(
                "track title cannot be empty".into(),
            ));
        }

        Ok(Self {
            title,
            artists: Vec::new(),
            album: None,
            duration: None,
            artwork_url: None,
            url: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_requires_an_application_name() {
        let error = MediaSource::browser("  ").unwrap_err();

        assert_eq!(
            error,
            Error::InvalidMediaData("media source application cannot be empty".into())
        );
    }

    #[test]
    fn track_starts_with_optional_metadata_absent() {
        let track = Track::new("Lateralus").unwrap();

        assert!(track.artists.is_empty());
        assert_eq!(track.duration, None);
        assert_eq!(track.artwork_url, None);
    }
}
