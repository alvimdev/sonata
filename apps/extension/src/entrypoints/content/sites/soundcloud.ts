import type { SiteAdapter } from './types';

/**
 * soundcloud.com. TODO: SoundCloud não expõe MediaSession de forma
 * confiável em todas as páginas — validar isso primeiro; se não der, vai
 * precisar ler o DOM do player fixo (título/artista/artwork ficam na barra
 * inferior) e escutar os botões de play/pause por MutationObserver.
 */
export const soundcloudAdapter: SiteAdapter = {
  matches(hostname) {
    return hostname === 'soundcloud.com';
  },

  observe(_onEvent) {
    console.warn('[sonata] adapter do SoundCloud ainda não implementado');
    return () => {};
  },
};