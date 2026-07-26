/**
 * Lista central de sites suportados. Alimenta o manifest (host_permissions),
 * o content script (matches) e a popup (exibição). Adicionar uma plataforma
 * nova começa e termina aqui + um novo adapter em `entrypoints/content/sites`.
 */
export const SUPPORTED_HOSTS = [
  'music.youtube.com',
  'soundcloud.com',
  'www.deezer.com',
] as const;

export type SupportedHost = (typeof SUPPORTED_HOSTS)[number];