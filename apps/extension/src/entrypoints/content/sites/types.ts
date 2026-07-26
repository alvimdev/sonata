import type { PlaybackState, Track } from '@/lib/protocol';

/**
 * Um adapter por site suportado. Mesmo objetivo de extensibilidade do
 * `MediaProvider` no lado Rust: adicionar uma plataforma nova deve significar
 * só escrever um adapter novo, nunca mexer no bootstrap do content script.
 */
export interface SiteAdapter {
  matches(hostname: string): boolean;

  /** Começa a observar a página, chama `onEvent` a cada mudança. Retorna cleanup. */
  observe(onEvent: (event: SiteEvent) => void): () => void;
}

export type SiteEvent =
  | { type: 'track'; track: Track }
  | { type: 'playback_state'; state: PlaybackState }
  | { type: 'position'; positionMs: number }
  | { type: 'ended' };