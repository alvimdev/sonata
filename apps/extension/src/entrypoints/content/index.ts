import { findAdapter } from './sites';
import type { SiteEvent } from './sites/types';
import type { MediaEventKind, MediaEventMessage } from '@/lib/protocol';
import { SUPPORTED_HOSTS } from '@/lib/sites';

export default defineContentScript({
  matches: SUPPORTED_HOSTS.map((host) => `https://${host}/*`),
  runAt: 'document_idle',

  main(ctx) {
    const adapter = findAdapter(location.hostname);
    if (!adapter) return;

    const stopObserving = adapter.observe((event) => {
      if (!ctx.isValid) return; // contexto já morreu, nem tenta

      const kind = toEventKind(event);
      if (!kind) return;

      const message: MediaEventMessage = {
        source: { kind: 'browser', application: location.hostname },
        kind,
      };

      try {
        browser.runtime.sendMessage(message).catch((error) => {
          console.warn('[sonata] falha ao enviar evento pro background', error);
        });
      } catch (error) {
        // sendMessage pode lançar de forma síncrona (não só rejeitar a
        // Promise) quando o contexto acabou de ser invalidado.
        console.warn('[sonata] contexto da extensão inválido, descartando evento', error);
      }
    });

    // Quando o WXT detecta que esse content script ficou órfão (extensão
    // desativada/recarregada), desliga os listeners do adapter de vez —
    // é isso que impede o `timeupdate` de continuar disparando depois.
    ctx.onInvalidated(stopObserving);
  },
});

function toEventKind(event: SiteEvent): MediaEventKind | null {
  switch (event.type) {
    case 'track':
      return { type: 'track_changed', track: event.track };
    case 'playback_state':
      return { type: 'playback_state_changed', state: event.state };
    case 'position':
      return { type: 'position_changed', positionMs: event.positionMs };
    case 'ended':
      return { type: 'session_ended' };
  }
}