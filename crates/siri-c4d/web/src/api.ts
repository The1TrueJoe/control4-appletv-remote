// Thin wrapper over the daemon's HTTP API. Every call returns parsed JSON or
// throws with the server's error message, so callers can just try/catch.

import type { Config, Discovered, Meta, StatusResponse } from './types.js';

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const response = await fetch(path, options);
  const body = await response.json().catch(() => ({}));
  if (!response.ok) {
    const message = (body as { error?: string }).error ?? `HTTP ${response.status}`;
    throw new Error(message);
  }
  return body as T;
}

function json(method: string, body: unknown): RequestInit {
  return {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  };
}

export const api = {
  status: () => request<StatusResponse>('/api/status'),
  getConfig: () => request<Config>('/api/config'),
  putConfig: (config: Config) => request<unknown>('/api/config', json('PUT', config)),
  meta: () => request<Meta>('/api/commands'),

  scan: (seconds = 20) =>
    request<{ devices: Discovered[] }>('/api/scan', json('POST', { seconds })),
  addRemote: (address: string, name: string) =>
    request<unknown>('/api/remotes', json('POST', { address, name })),
  removeRemote: (address: string) =>
    request<unknown>(`/api/remotes/${encodeURIComponent(address)}`, { method: 'DELETE' }),
  testControl: (address: string, control: string) =>
    request<unknown>(
      `/api/remotes/${encodeURIComponent(address)}/test`,
      json('POST', { control }),
    ),

  c4Health: () => request<unknown>('/api/c4/health'),
  c4Rooms: () => request<unknown>('/api/c4/rooms'),
};
