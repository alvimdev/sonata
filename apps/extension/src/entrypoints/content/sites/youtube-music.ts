import type { SiteAdapter } from './types';

/**
 * music.youtube.com. Usa a MediaSession API da própria página (o YT Music já
 * normaliza título/artista/artwork ali) e só recorre ao DOM/URL pra o que a
 * MediaSession não expõe, como a URL canônica da faixa.
 */
export const youtubeMusicAdapter: SiteAdapter = {
  matches(hostname) {
    return hostname === 'music.youtube.com';
  },

  observe(onEvent) {
    const video = document.querySelector('video');
    if (!video) return () => {};

    const emitTrack = () => {
      const metadata = navigator.mediaSession?.metadata;
      if (!metadata) return;

      onEvent({
        type: 'track',
        track: {
          title: metadata.title,
          artists: metadata.artist ? [metadata.artist] : [],
          album: metadata.album || null,
          durationMs: Number.isFinite(video.duration) ? video.duration * 1000 : null,
          artworkUrl: metadata.artwork?.at(-1)?.src ?? null,
          url: canonicalTrackUrl(),
        },
      });
    };

    const emitPlaybackState = () => {
      onEvent({ type: 'playback_state', state: video.paused ? 'paused' : 'playing' });
    };

    const emitPosition = () => {
      onEvent({ type: 'position', positionMs: video.currentTime * 1000 });
    };

    // A troca de faixa atualiza o <title> da aba junto com a MediaSession.
    const titleEl = document.querySelector('title');
    const metadataObserver = new MutationObserver(emitTrack);
    if (titleEl) metadataObserver.observe(titleEl, { childList: true });

    video.addEventListener('play', emitPlaybackState);
    video.addEventListener('pause', emitPlaybackState);
    video.addEventListener('timeupdate', emitPosition);
    video.addEventListener('ended', () => onEvent({ type: 'ended' }));

    emitTrack();
    emitPlaybackState();
    emitPosition();

    return () => {
      metadataObserver.disconnect();
      video.removeEventListener('play', emitPlaybackState);
      video.removeEventListener('pause', emitPlaybackState);
      video.removeEventListener('timeupdate', emitPosition);
    };
  },
};

function canonicalTrackUrl(): string | null {
  const videoId = new URLSearchParams(location.search).get('v');
  return videoId ? `https://music.youtube.com/watch?v=${videoId}` : null;
}