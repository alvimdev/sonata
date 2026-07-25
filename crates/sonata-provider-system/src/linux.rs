use std::{collections::HashMap, time::Duration};

use futures_util::StreamExt;
use sonata_core::{Error, MediaEvent, MediaEventKind, MediaSource, PlaybackState, Result, Track};
use tokio::sync::mpsc;
use zbus::{Connection, Proxy, fdo::PropertiesProxy, zvariant::OwnedValue};

const MPRIS_OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
const MPRIS_PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";

/// Linux implementation backed by MPRIS over the session D-Bus.
pub(super) struct PlatformProvider {
    events: mpsc::Receiver<Result<MediaEvent>>,
}

impl PlatformProvider {
    pub(super) async fn connect(player: Option<String>) -> Result<Self> {
        let player = player.ok_or_else(|| {
            Error::InvalidMediaData(
                "Linux system provider requires a MPRIS player name".into(),
            )
        })?;

        if !player.starts_with("org.mpris.MediaPlayer2.") {
            return Err(Error::InvalidMediaData(
                "Linux MPRIS player names must start with org.mpris.MediaPlayer2.".into(),
            ));
        }

        let source = MediaSource::system(player.clone())?;
        let connection = Connection::session().await.map_err(|error| {
            Error::Provider(format!("could not connect to the session D-Bus: {error}"))
        })?;
        let player_proxy = Proxy::new(
            &connection,
            player.clone(),
            MPRIS_OBJECT_PATH,
            MPRIS_PLAYER_INTERFACE,
        )
        .await
        .map_err(|error| {
            Error::Provider(format!(
                "could not connect to MPRIS player {player}: {error}"
            ))
        })?;
        let properties = PropertiesProxy::new(&connection, player, MPRIS_OBJECT_PATH)
            .await
            .map_err(|error| {
                Error::Provider(format!("could not observe MPRIS properties: {error}"))
            })?;
        let (sender, events) = mpsc::channel(32);

        tokio::spawn(observe(
            player_proxy.clone(),
            properties,
            source.clone(),
            sender.clone(),
        ));
        tokio::spawn(observe_session_end(player_proxy, source, sender));

        Ok(Self { events })
    }

    pub(super) async fn next_event(&mut self) -> Result<MediaEvent> {
        self.events.recv().await.unwrap_or_else(|| {
            Err(Error::Provider(
                "the Linux MPRIS event observer stopped unexpectedly".into(),
            ))
        })
    }
}

async fn observe(
    player: Proxy<'static>,
    properties: PropertiesProxy<'static>,
    source: MediaSource,
    sender: mpsc::Sender<Result<MediaEvent>>,
) {
    if let Err(error) = send_initial_events(&player, &source, &sender).await {
        let _ = sender.send(Err(error)).await;
        return;
    }

    let mut changes = match properties.receive_properties_changed().await {
        Ok(changes) => changes,
        Err(error) => {
            let _ = sender
                .send(Err(Error::Provider(format!(
                    "could not subscribe to MPRIS property changes: {error}"
                ))))
                .await;
            return;
        }
    };

    while let Some(change) = changes.next().await {
        let arguments = match change.args() {
            Ok(arguments) => arguments,
            Err(error) => {
                let _ = sender
                    .send(Err(Error::Provider(format!(
                        "could not read an MPRIS property change: {error}"
                    ))))
                    .await;
                continue;
            }
        };

        if arguments.interface_name.as_str() != MPRIS_PLAYER_INTERFACE {
            continue;
        }

        let changed_properties = arguments
            .changed_properties
            .iter()
            .map(|(name, value)| {
                OwnedValue::try_from(value.clone())
                    .map(|value| (name.to_string(), value))
                    .map_err(|error| {
                        Error::Provider(format!(
                            "could not own an MPRIS property change value: {error}"
                        ))
                    })
            })
            .collect::<Result<HashMap<_, _>>>();

        let changed_properties = match changed_properties {
            Ok(changed_properties) => changed_properties,
            Err(error) => {
                if sender.send(Err(error)).await.is_err() {
                    return;
                }
                continue;
            }
        };

        for event in events_from_properties(&source, &changed_properties) {
            if sender.send(event).await.is_err() {
                return;
            }
        }
    }
}

async fn observe_session_end(
    player: Proxy<'static>,
    source: MediaSource,
    sender: mpsc::Sender<Result<MediaEvent>>,
) {
    let mut owner_changes = match player.receive_owner_changed().await {
        Ok(owner_changes) => owner_changes,
        Err(error) => {
            let _ = sender
                .send(Err(Error::Provider(format!(
                    "could not observe the MPRIS player owner: {error}"
                ))))
                .await;
            return;
        }
    };

    while let Some(owner) = owner_changes.next().await {
        if owner.is_none()
            && sender
                .send(Ok(MediaEvent::session_ended(source.clone())))
                .await
                .is_err()
        {
            return;
        }
    }
}

async fn send_initial_events(
    player: &Proxy<'_>,
    source: &MediaSource,
    sender: &mpsc::Sender<Result<MediaEvent>>,
) -> Result<()> {
    let metadata = player
        .get_property::<HashMap<String, OwnedValue>>("Metadata")
        .await
        .map_err(|error| Error::Provider(format!("could not read MPRIS metadata: {error}")))?;
    let status = player
        .get_property::<String>("PlaybackStatus")
        .await
        .map_err(|error| {
            Error::Provider(format!("could not read MPRIS playback status: {error}"))
        })?;

    let track = track_from_metadata(&OwnedValue::from(metadata))?;
    sender
        .send(Ok(MediaEvent::track_changed(source.clone(), track)))
        .await
        .map_err(|_| Error::Provider("MPRIS event receiver was dropped".into()))?;
    sender
        .send(Ok(MediaEvent::playback_state_changed(
            source.clone(),
            playback_state_name(&status)?,
        )))
        .await
        .map_err(|_| Error::Provider("MPRIS event receiver was dropped".into()))?;

    Ok(())
}

fn events_from_properties(
    source: &MediaSource,
    properties: &HashMap<String, OwnedValue>,
) -> Vec<Result<MediaEvent>> {
    let mut events = Vec::new();

    if let Some(metadata) = properties.get("Metadata") {
        events.push(track_from_metadata(metadata).map(|track| MediaEvent {
            source: source.clone(),
            kind: MediaEventKind::TrackChanged { track },
        }));
    }

    if let Some(status) = properties.get("PlaybackStatus") {
        events.push(playback_state(status).map(|state| MediaEvent {
            source: source.clone(),
            kind: MediaEventKind::PlaybackStateChanged { state },
        }));
    }

    if let Some(position) = properties.get("Position") {
        events.push(position_from_micros(position).map(|position| MediaEvent {
            source: source.clone(),
            kind: MediaEventKind::PositionChanged { position },
        }));
    }

    events
}

fn track_from_metadata(metadata: &OwnedValue) -> Result<Track> {
    let metadata = HashMap::<String, OwnedValue>::try_from(metadata.clone())
        .map_err(|error| Error::Provider(format!("could not decode MPRIS metadata: {error}")))?;
    let title = required_string(&metadata, "xesam:title")?;
    let artists = metadata
        .get("xesam:artist")
        .map(string_list)
        .transpose()?
        .unwrap_or_default();
    let album = metadata.get("xesam:album").map(string_value).transpose()?;
    let duration = metadata
        .get("mpris:length")
        .map(position_from_micros)
        .transpose()?;
    let artwork_url = metadata.get("mpris:artUrl").map(string_value).transpose()?;

    Ok(Track {
        title,
        artists,
        album,
        duration,
        artwork_url,
    })
}

fn playback_state(value: &OwnedValue) -> Result<PlaybackState> {
    playback_state_name(&string_value(value)?)
}

fn playback_state_name(status: &str) -> Result<PlaybackState> {
    match status {
        "Playing" => Ok(PlaybackState::Playing),
        "Paused" => Ok(PlaybackState::Paused),
        "Stopped" => Ok(PlaybackState::Stopped),
        status => Err(Error::Provider(format!(
            "received unknown MPRIS playback status: {status}"
        ))),
    }
}

fn position_from_micros(value: &OwnedValue) -> Result<Duration> {
    let micros = i64::try_from(value.clone())
        .map_err(|error| Error::Provider(format!("could not decode MPRIS position: {error}")))?;
    let micros = u64::try_from(micros)
        .map_err(|_| Error::Provider("received a negative MPRIS position".into()))?;

    Ok(Duration::from_micros(micros))
}

fn required_string(metadata: &HashMap<String, OwnedValue>, key: &str) -> Result<String> {
    metadata
        .get(key)
        .ok_or_else(|| Error::Provider(format!("MPRIS metadata is missing {key}")))
        .and_then(string_value)
}

fn string_value(value: &OwnedValue) -> Result<String> {
    String::try_from(value.clone())
        .map_err(|error| Error::Provider(format!("could not decode MPRIS string: {error}")))
}

fn string_list(value: &OwnedValue) -> Result<Vec<String>> {
    Vec::<String>::try_from(value.clone())
        .map_err(|error| Error::Provider(format!("could not decode MPRIS artist list: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_mpris_playback_states() {
        assert_eq!(
            playback_state_name("Playing").unwrap(),
            PlaybackState::Playing
        );
    }

    #[test]
    fn converts_mpris_microseconds_to_a_duration() {
        let value = OwnedValue::from(1_500_000_i64);

        assert_eq!(
            position_from_micros(&value).unwrap(),
            Duration::from_millis(1500)
        );
    }
}
