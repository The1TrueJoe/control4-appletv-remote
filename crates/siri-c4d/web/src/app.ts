// Wiring. All interaction is delegated from one listener, so re-rendered
// markup needs no re-binding and the HTML carries no inline handlers.

import { api } from './api.js';
import { esc, flash, must } from './dom.js';
import { clearLog, connectEvents } from './log.js';
import { pair, scan } from './pairing.js';
import { bindRemoteEdits, removeRemote, renderRemotes, testControl } from './remotes.js';
import { refreshStatus } from './status.js';
import { config, load, save } from './store.js';

/** Push form fields into the in-memory config, then persist. */
async function saveAll(): Promise<void> {
  const cfg = config();
  cfg.control4.host = must<HTMLInputElement>('#c4host').value.trim();
  cfg.control4.port = Number(must<HTMLInputElement>('#c4port').value) || 5080;
  cfg.control4.password = must<HTMLInputElement>('#c4pass').value;
  cfg.control4.room = must<HTMLInputElement>('#c4room').value.trim();
  cfg.voice.audio_dir = must<HTMLInputElement>('#audiodir').value.trim();
  cfg.voice.enabled = must<HTMLInputElement>('#voiceon').checked;
  cfg.voice.keep_recordings = must<HTMLInputElement>('#keepwav').checked;

  try {
    await save();
    flash('#c4msg', 'saved', 'ok');
    await refreshStatus();
  } catch (error) {
    flash('#c4msg', (error as Error).message, 'bad');
  }
}

/** Populate the forms from the loaded config. */
function renderForms(): void {
  const cfg = config();
  must<HTMLInputElement>('#c4host').value = cfg.control4.host;
  must<HTMLInputElement>('#c4port').value = String(cfg.control4.port);
  must<HTMLInputElement>('#c4pass').value = cfg.control4.password;
  must<HTMLInputElement>('#c4room').value = cfg.control4.room;
  must<HTMLInputElement>('#audiodir').value = cfg.voice.audio_dir;
  must<HTMLInputElement>('#voiceon').checked = cfg.voice.enabled;
  must<HTMLInputElement>('#keepwav').checked = cfg.voice.keep_recordings;
}

async function reload(): Promise<void> {
  await load();
  renderForms();
  renderRemotes();
  await refreshStatus();
}

async function onAction(action: string, target: HTMLElement): Promise<void> {
  const card = target.closest<HTMLElement>('[data-remote]');
  const address = card?.dataset.remote ?? '';

  switch (action) {
    case 'save':
      await saveAll();
      break;
    case 'scan':
      await scan();
      break;
    case 'pair':
      await pair(target.dataset.address ?? '', target.dataset.name ?? '', reload);
      break;
    case 'remove-remote':
      await removeRemote(address, reload);
      break;
    case 'test-remote':
      await testControl(address);
      break;
    case 'clear-log':
      clearLog();
      break;
    case 'c4-health':
      try {
        flash('#c4msg', `reachable: ${JSON.stringify(await api.c4Health())}`, 'ok');
      } catch (error) {
        flash('#c4msg', (error as Error).message, 'bad');
      }
      break;
    case 'c4-rooms':
      try {
        flash('#c4msg', JSON.stringify(await api.c4Rooms()), 'ok');
      } catch (error) {
        flash('#c4msg', (error as Error).message, 'bad');
      }
      break;
    default:
      break;
  }
}

function main(): void {
  document.body.addEventListener('click', (event) => {
    const target = (event.target as HTMLElement).closest<HTMLElement>('[data-action]');
    if (!target) return;
    void onAction(target.dataset.action ?? '', target);
  });

  bindRemoteEdits(must('#remotes'));
  connectEvents(() => void refreshStatus());

  reload().catch((error: Error) => {
    must('#hdr').textContent = `error: ${esc(error.message)}`;
  });

  window.setInterval(() => void refreshStatus(), 5000);
}

main();
