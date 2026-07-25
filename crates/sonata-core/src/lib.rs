//! Domain contracts shared by Sonata providers, publishers, and applications.
//!
//! This crate intentionally contains no platform or transport details.

#![forbid(unsafe_code)]

mod error;
mod event;
mod model;
mod provider;
mod publisher;

pub use error::{Error, Result};
pub use event::{MediaEvent, MediaEventKind};
pub use model::{MediaSource, MediaSourceKind, PlaybackState, Track};
pub use provider::MediaProvider;
pub use publisher::PresencePublisher;
