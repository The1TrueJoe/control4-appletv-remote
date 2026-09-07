// Scanning for and pairing remotes.

import { api } from './api.js';
import { esc, must, spinner } from './dom.js';
import type { Discovered } from './types.js';

function resultRow(device: Discovered): string {
  const likely = device.likely_remote ? ' <span class="chip ok">likely</span>' : '';
  const name = device.name ? esc(device.name) : '<span class="sub">unnamed</span>';
  return `
    <tr>
      <td>${esc(device.address)}${likely}</td>
      <td>${name}</td>
      <td>${device.rssi ?? '–'} dBm</td>
      <td>${esc(device.addr_type)}</td>
      <td><button data-action="pair" data-address="${esc(device.address)}"
                  data-name="${esc(device.name)}">Pair</button></td>
    </tr>`;
}

export async function scan(): Promise<void> {
  const button = must<HTMLButtonElement>('#scanBtn');
  button.disabled = true;
  must('#scanmsg').innerHTML = spinner('scanning 20s — press a button on the remote');

  try {
    const { devices } = await api.scan(20);
    must('#found tbody').innerHTML = devices.map(resultRow).join('');
    must('#found').hidden = devices.length === 0;
    must('#scanmsg').textContent = devices.length
      ? `${devices.length} device(s) found`
      : 'nothing found — press a button during the scan, or hold Menu + Volume Up';
  } catch (error) {
    must('#scanmsg').textContent = (error as Error).message;
  } finally {
    button.disabled = false;
  }
}

export async function pair(
  address: string,
  name: string,
  reload: () => Promise<void>,
): Promise<void> {
  must('#scanmsg').innerHTML = spinner('pairing…');
  try {
    await api.addRemote(address, name);
    must('#scanmsg').textContent = 'paired';
    await reload();
  } catch (error) {
    must('#scanmsg').textContent = (error as Error).message;
  }
}
