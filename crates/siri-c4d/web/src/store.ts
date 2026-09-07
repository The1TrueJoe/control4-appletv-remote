// Shared state: the config being edited, and the daemon's metadata about which
// controls and commands exist. One place, so modules neither pass it around
// nor re-fetch it.

import { api } from './api.js';
import type { Config, Meta } from './types.js';

interface Store {
  config: Config | null;
  meta: Meta | null;
}

const state: Store = { config: null, meta: null };

/** The config is only null before the first load; accessors assert that so
 *  callers are not littered with null checks. */
export function config(): Config {
  if (!state.config) throw new Error('config not loaded yet');
  return state.config;
}

export function meta(): Meta {
  if (!state.meta) throw new Error('metadata not loaded yet');
  return state.meta;
}

export async function load(): Promise<void> {
  const [cfg, m] = await Promise.all([api.getConfig(), api.meta()]);
  state.config = cfg;
  state.meta = m;
}

export async function save(): Promise<void> {
  await api.putConfig(config());
}
