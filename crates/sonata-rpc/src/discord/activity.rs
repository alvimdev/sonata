use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, Assets, Timestamps};
use sonata_core::{PlaybackState, Track};

const ACTIVITY_NAME: &str = "Sonata";
const SONATA_BADGE: &str = "sonata";
const FALLBACK_ARTWORK: &str = "leon";

pub fn build_activity(
    track: &Track,
    playback_state: PlaybackState,
    position: Option<Duration>,
) -> Option<Activity<'static>> {
    let large_image: String = track
        .artwork_url
        .clone()
        .unwrap_or_else(|| FALLBACK_ARTWORK.to_string());

    let assets = Assets::new()
        .large_image(large_image)
        .large_text(large_text(track))
        .small_image(SONATA_BADGE)
        .small_text(ACTIVITY_NAME);

    let mut activity = Activity::new()
        .name(ACTIVITY_NAME)
        .details(track.title.clone())
        .state(state_text(track))
        .assets(assets)
        .activity_type(ActivityType::Listening);

    if playback_state == PlaybackState::Playing
        && let (Some(duration), Some(position)) = (track.duration, position)
        && let Some(timestamps) = timestamps(duration, position)
    {
        activity = activity.timestamps(timestamps);
    }

    Some(activity)
}

fn state_text(track: &Track) -> String {
    track
        .artists
        .first()
        .map(|artist| format!("by {artist}"))
        .unwrap_or_else(|| "by Unknown Artist".to_string())
}

fn large_text(track: &Track) -> String {
    track
        .album
        .clone()
        .or_else(|| track.artists.first().cloned())
        .unwrap_or_else(|| "Unknown Artist".to_string())
}

fn timestamps(duration: Duration, position: Duration) -> Option<Timestamps> {
    let now = unix_millis()?;
    let duration = duration_to_millis(duration)?;
    let position = duration_to_millis(position)?;
    let start = now.checked_sub(position)?;
    let end = start.checked_add(duration)?;

    Some(Timestamps::new().start(start).end(end))
}

fn unix_millis() -> Option<i64> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_millis()).ok()
}

fn duration_to_millis(duration: Duration) -> Option<i64> {
    i64::try_from(duration.as_millis()).ok()
}