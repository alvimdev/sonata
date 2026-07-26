import type { SiteAdapter } from './types';

/** www.deezer.com. TODO: mesma investigação do SoundCloud. */
export const deezerAdapter: SiteAdapter = {
  matches(hostname) {
    return hostname === 'www.deezer.com';
  },

  observe(_onEvent) {
    console.warn('[sonata] adapter do Deezer ainda não implementado');
    return () => {};
  },
};