// Paired remotes: their settings and their control mappings.

import { api } from './api.js';
import { esc, must } from './dom.js';
import { log } from './log.js';
import { config, meta } from './store.js';
import type { Action, Remote } from './types.js';

/** Encode an action as a <select> value, and decode it back. Keeping the two
 *  adjacent makes it obvious they must agree. */
function actionValue(action: Action): string {
  switch (action.kind) {
    case 'command': return `cmd:${action.command}`;
    case 'hold': return `hold:${action.start}`;
    default: return 'none';
  }
}

export function parseAction(value: string): Action {
  if (value.startsWith('cmd:')) {
    return { kind: 'command', command: value.slice(4) };
  }
  if (value.startsWith('hold:')) {
    const start = value.slice(5);
    const pair = Object.values(meta().hold_pairs).find((p) => p.start === start);
    if (pair) return { kind: 'hold', start: pair.start, stop: pair.stop };
  }
  return { kind: 'none' };
}

/** Offer plain commands plus the Control4 hold pairs, which ramp while held. */
function actionSelect(control: string, current: Action): string {
  const selected = actionValue(current);

  const commands = meta().commands.map((command) => {
    const value = `cmd:${command}`;
    return `<option value="${esc(value)}" ${value === selected ? 'selected' : ''}>${esc(command)}</option>`;
  }).join('');

  const holds = Object.entries(meta().hold_pairs).map(([name, pair]) => {
    const value = `hold:${pair.start}`;
    return `<option value="${esc(value)}" ${value === selected ? 'selected' : ''}>${esc(name)} (hold)</option>`;
  }).join('');

  return `
    <select data-mapping="${esc(control)}">
      <option value="none" ${selected === 'none' ? 'selected' : ''}>— nothing —</option>
      <optgroup label="Command">${commands}</optgroup>
      <optgroup label="Hold (ramps while held)">${holds}</optgroup>
    </select>`;
}

function mappingsHtml(remote: Remote): string {
  return meta().controls.map((control) => {
    const current: Action = remote.mappings?.[control] ?? { kind: 'none' };
    const hint = current.kind === 'hold' ? 'press + release' : '';
    return `
      <div class="mapping">
        <span class="sub">${esc(control)}</span>
        <span>${actionSelect(control, current)}</span>
        <span class="sub">${hint}</span>
      </div>`;
  }).join('');
}

function remoteCard(remote: Remote, index: number): string {
  const id = remote.address.replace(/:/g, '');
  return `
    <div class="card" data-remote="${esc(remote.address)}" data-index="${index}"
         style="margin-bottom:12px">
      <div class="row" style="justify-content:space-between">
        <div>
          <strong>${esc(remote.name || remote.address)}</strong>
          <div class="sub">${esc(remote.address)} · ${esc(remote.addr_type)}</div>
        </div>
        <div id="st-${esc(id)}"></div>
      </div>

      <div class="grid" style="margin-top:12px">
        <div><label>Name</label>
          <input value="${esc(remote.name)}" data-field="name"></div>
        <div><label>Room override</label>
          <input value="${esc(remote.room)}" placeholder="use default" data-field="room"></div>
        <div><label>Enabled</label>
          <input type="checkbox" ${remote.enabled ? 'checked' : ''} data-field="enabled"></div>
      </div>

      <details style="margin-top:12px">
        <summary>Button mappings</summary>
        <div style="margin-top:10px">${mappingsHtml(remote)}</div>
      </details>

      <div class="row" style="margin-top:12px">
        <button data-action="save">Save</button>
        <button class="ghost" data-action="test-remote">Test mapping…</button>
        <button class="danger" data-action="remove-remote">Remove</button>
      </div>
    </div>`;
}

export function renderRemotes(): void {
  const remotes = config().remotes ?? [];
  must('#remotes').innerHTML = remotes.map(remoteCard).join('');
  must('#noremotes').hidden = remotes.length > 0;
}

function remoteFor(target: HTMLElement): Remote | null {
  const card = target.closest<HTMLElement>('[data-remote]');
  if (!card) return null;
  const index = Number(card.dataset.index);
  return config().remotes[index] ?? null;
}

/** Field edits mutate the in-memory config; nothing persists until Save. */
export function bindRemoteEdits(root: HTMLElement): void {
  root.addEventListener('input', (event) => {
    const target = event.target as HTMLInputElement;
    const field = target.dataset.field;
    const remote = remoteFor(target);
    if (!field || !remote) return;

    if (field === 'enabled') remote.enabled = target.checked;
    else if (field === 'name') remote.name = target.value;
    else if (field === 'room') remote.room = target.value;
  });

  root.addEventListener('change', (event) => {
    const target = event.target as HTMLSelectElement;
    const control = target.dataset.mapping;
    const remote = remoteFor(target);
    if (!control || !remote) return;
    remote.mappings[control] = parseAction(target.value);
  });
}

export async function removeRemote(
  address: string,
  reload: () => Promise<void>,
): Promise<void> {
  if (!window.confirm(`Remove ${address}? This also unpairs it.`)) return;
  await api.removeRemote(address);
  await reload();
}

export async function testControl(address: string): Promise<void> {
  const control = window.prompt('Which control? e.g. MENU, VOLUME_UP, SWIPE_LEFT', 'MENU');
  if (!control) return;
  try {
    await api.testControl(address, control);
    log(`test: sent ${control}`);
  } catch (error) {
    log(`test failed: ${(error as Error).message}`);
  }
}
