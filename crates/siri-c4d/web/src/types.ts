// Types mirroring the daemon's Rust structs. Keeping these in sync is the
// point of using TypeScript here: a renamed field fails the build instead of
// silently rendering "undefined".

/** `config::Action` — what a control on the remote should do. */
export type Action =
  | { kind: 'none' }
  | { kind: 'command'; command: string }
  | { kind: 'hold'; start: string; stop: string }
  | { kind: 'preset'; preset: string }
  | { kind: 'button'; button: string; action: string };

/** `config::Remote` */
export interface Remote {
  address: string;
  name: string;
  enabled: boolean;
  addr_type: string;
  /** Empty means fall back to `Control4.room`. */
  room: string;
  mappings: Record<string, Action>;
}

/** `config::Control4` */
export interface Control4Config {
  host: string;
  port: number;
  password: string;
  room: string;
  timeout_ms: number;
}

/** `config::Voice` */
export interface VoiceConfig {
  enabled: boolean;
  audio_dir: string;
  keep_recordings: boolean;
}

/** `config::Config` */
export interface Config {
  web: { bind: string; port: number };
  control4: Control4Config;
  voice: VoiceConfig;
  remotes: Remote[];
}

/** `supervisor::RemoteStatus` */
export interface RemoteStatus {
  connected: boolean;
  mtu: number;
  audio_available: boolean;
  battery: number | null;
  power: string | null;
  last_event: string | null;
  last_error: string | null;
}

/** `bluez::AdapterInfo` */
export interface AdapterInfo {
  address: string;
  name: string;
  powered: boolean;
  discovering: boolean;
}

export interface StatusResponse {
  remotes: Array<{
    address: string;
    name: string;
    enabled: boolean;
    room: string;
    status: RemoteStatus | null;
  }>;
  adapter: AdapterInfo | null;
  control4: { host: string; port: number; configured: boolean };
  voice: { enabled: boolean; keep_recordings: boolean; audio_dir: string };
  version: string;
}

/** Controls and commands come from the daemon so the Control4 vocabulary is
 *  never hard-coded in the UI. */
export interface Meta {
  controls: string[];
  commands: string[];
  hold_pairs: Record<string, { start: string; stop: string }>;
}

/** `bluez::Discovered` */
export interface Discovered {
  address: string;
  name: string;
  rssi: number | null;
  paired: boolean;
  addr_type: string;
  likely_remote: boolean;
}

/** Events pushed over SSE. Only the fields each kind actually carries are
 *  guaranteed, hence the optional members. */
export interface LiveEvent {
  kind: string;
  address: string;
  at: string;
  button?: string;
  state?: string;
  swipe?: string;
  control?: string;
  command?: string;
  ok?: boolean;
  error?: string;
  mtu?: number;
  needed?: number;
  percent?: number;
  frames?: number;
  seconds?: number;
  path?: string;
}
