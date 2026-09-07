// The status panel, plus the per-remote status chips.

import { api } from './api.js';
import { esc, must } from './dom.js';
import type { AdapterInfo, RemoteStatus } from './types.js';

function adapterCell(adapter: AdapterInfo | null): string {
  if (!adapter) return '<span class="bad">not available</span>';
  const detail = [adapter.powered ? 'powered' : 'off'];
  if (adapter.discovering) detail.push('scanning');
  return `<span class="ok">${esc(adapter.address)}</span>
          <div class="sub">${detail.join(', ')}</div>`;
}

function remoteChips(status: RemoteStatus | null): string {
  if (!status) return '<span class="sub">idle</span>';

  const chips = [
    status.connected
      ? '<span class="chip ok">connected</span>'
      : '<span class="chip">waiting — press a button</span>',
  ];
  if (status.battery != null) chips.push(`<span class="chip">${status.battery}%</span>`);
  if (status.power) chips.push(`<span class="chip">${esc(status.power)}</span>`);

  if (status.connected && status.mtu) {
    chips.push(status.audio_available
      ? `<span class="chip ok">audio ok (MTU ${status.mtu})</span>`
      : `<span class="chip warn" title="The remote refuses a larger MTU to controllers without Bluetooth 4.2 Data Length Extension">no audio (MTU ${status.mtu})</span>`);
  }
  if (status.last_error) {
    chips.push(`<span class="chip bad" title="${esc(status.last_error)}">error</span>`);
  }
  return chips.join(' ');
}

export async function refreshStatus(): Promise<void> {
  let state;
  try {
    state = await api.status();
  } catch {
    must('#hdr').textContent = 'daemon unreachable';
    return;
  }

  must('#hdr').textContent = `v${state.version}`;
  const connected = state.remotes.filter((r) => r.status?.connected).length;

  must('#status').innerHTML = `
    <div><label>Bluetooth adapter</label>${adapterCell(state.adapter)}</div>
    <div><label>Control4</label>
      ${state.control4.configured
        ? `<span class="ok">${esc(state.control4.host)}:${state.control4.port}</span>`
        : '<span class="warn">not configured</span>'}</div>
    <div><label>Remotes</label>${state.remotes.length}
      <div class="sub">${connected} connected</div></div>
    <div><label>Voice</label>
      ${state.voice.enabled
        ? '<span class="ok">capturing</span>'
        : '<span class="sub">disabled</span>'}</div>`;

  for (const remote of state.remotes) {
    const el = document.getElementById(`st-${remote.address.replace(/:/g, '')}`);
    if (el) el.innerHTML = remoteChips(remote.status);
  }
}
