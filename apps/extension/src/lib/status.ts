export type TrackState = 'playing' | 'paused' | 'stopped';

export interface StatusTrack {
  title: string;
  artists: string[];
  album: string | null;
  artworkUrl: string | null;
  state: TrackState;
}

export interface DaemonStatus {
  connected: boolean;
  track: StatusTrack | null;
}

export const STATUS_REQUEST_TYPE = 'sonata:get-status';

export interface StatusRequestMessage {
  type: typeof STATUS_REQUEST_TYPE;
}