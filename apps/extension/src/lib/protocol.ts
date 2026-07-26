/**
 * Espelha `Track`, `PlaybackState`, `MediaSource` e `MediaEvent` de
 * crates/sonata-core/src/{model,event}.rs. Este é o contrato de wire entre
 * a extensão e o daemon — mantenha em sincronia manualmente até existir
 * geração automática (ex: via schema compartilhado).
 */

export type PlaybackState = 'playing' | 'paused' | 'stopped';

export interface Track {
  title: string;
  artists: string[];
  album: string | null;
  durationMs: number | null;
  artworkUrl: string | null;
  url: string | null;
}

export interface MediaSource {
  kind: 'browser';
  application: string; // hostname do site, ex: "music.youtube.com"
}

export type MediaEventKind =
  | { type: 'track_changed'; track: Track }
  | { type: 'playback_state_changed'; state: PlaybackState }
  | { type: 'position_changed'; positionMs: number }
  | { type: 'session_ended' };

export interface MediaEventMessage {
  source: MediaSource;
  kind: MediaEventKind;
}