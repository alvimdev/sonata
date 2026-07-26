import type { SiteAdapter } from './types';
import { youtubeMusicAdapter } from './youtube-music';
import { soundcloudAdapter } from './soundcloud';
import { deezerAdapter } from './deezer';

export const adapters: SiteAdapter[] = [youtubeMusicAdapter, soundcloudAdapter, deezerAdapter];

export function findAdapter(hostname: string): SiteAdapter | undefined {
  return adapters.find((adapter) => adapter.matches(hostname));
}