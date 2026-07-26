import type { MediaEventMessage } from '@/lib/protocol';
import type { DaemonStatus, StatusRequestMessage } from '@/lib/status';
import { STATUS_REQUEST_TYPE } from '@/lib/status';
import { NATIVE_HOST_NAME } from '@/lib/host';

type NativePort = ReturnType<typeof browser.runtime.connectNative>;

const RECONNECT_COOLDOWN_MS = 5000;

export default defineBackground(() => {
  let port: NativePort | null = null;
  let lastFailureAt = 0;

  const status: DaemonStatus = { connected: false, track: null };

  function getPort(): NativePort | null {
    if (port) return port;
    if (Date.now() - lastFailureAt < RECONNECT_COOLDOWN_MS) return null;

    const nextPort = browser.runtime.connectNative(NATIVE_HOST_NAME);

    nextPort.onDisconnect.addListener(() => {
      const error = browser.runtime.lastError;
      if (error) console.warn('[sonata] native host desconectou:', error.message);
      port = null;
      lastFailureAt = Date.now();
      status.connected = false;
    });

    port = nextPort;
    status.connected = true;
    return port;
  }

  function applyEvent(message: MediaEventMessage) {
    switch (message.kind.type) {
      case 'track_changed':
        status.track = {
          title: message.kind.track.title,
          artists: message.kind.track.artists,
          album: message.kind.track.album,
          artworkUrl: message.kind.track.artworkUrl,
          state: status.track?.state ?? 'playing',
        };
        break;
      case 'playback_state_changed':
        if (status.track) status.track.state = message.kind.state;
        break;
      case 'session_ended':
        status.track = null;
        break;
      case 'position_changed':
        break; // não afeta o que a popup mostra
    }
  }

  browser.runtime.onMessage.addListener((message: MediaEventMessage | StatusRequestMessage) => {
    if ('type' in message && message.type === STATUS_REQUEST_TYPE) {
      return Promise.resolve(status);
    }

    const mediaMessage = message as MediaEventMessage;
    applyEvent(mediaMessage);

    try {
      getPort()?.postMessage(mediaMessage);
    } catch (error) {
      console.warn('[sonata] falha ao enviar evento pro daemon:', error);
      port = null;
      lastFailureAt = Date.now();
      status.connected = false;
    }
  });
});