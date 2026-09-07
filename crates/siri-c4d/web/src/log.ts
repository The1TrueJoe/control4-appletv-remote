// The live event feed: an SSE subscription rendered into a scrolling log.

import { $ } from './dom.js';
import type { LiveEvent } from './types.js';

export function log(line: string): void {
  const el = $('#log');
  if (!el) return;
  const stamp = new Date().toLocaleTimeString();
  el.textContent += `[${stamp}] ${line}\n`;
  el.scrollTop = el.scrollHeight;
}

export function clearLog(): void {
  const el = $('#log');
  if (el) el.textContent = '';
}

/** Render one event, or null to ignore it. Unknown kinds are dropped rather
 *  than logged as noise, so adding events to the daemon needs no change here. */
function describe(event: LiveEvent): string | null {
  const who = (event.address ?? '').slice(-5);
  switch (event.kind) {
    case 'button':
      return `${who}  ${event.button} ${event.state}`;
    case 'swipe':
      return `${who}  ${event.swipe}`;
    case 'command': {
      const detail = event.command ? ` ${event.command}` : '';
      return `${who}  → ${event.control}${detail} ` +
        (event.ok ? 'ok' : `FAILED: ${event.error}`);
    }
    case 'connected':
      return `${who}  connected (MTU ${event.mtu})`;
    case 'disconnected':
      return `${who}  disconnected`;
    case 'audio_unavailable':
      return `${who}  no microphone: MTU ${event.mtu} < ${event.needed} ` +
        '(needs a Bluetooth 4.2+ adapter)';
    case 'voice_start':
      return `${who}  listening…`;
    case 'voice_end':
      return `${who}  voice: ${event.frames} frames, ${(event.seconds ?? 0).toFixed(2)}s`;
    case 'voice_saved':
      return `${who}  saved ${event.path}`;
    case 'voice_error':
      return `${who}  voice error: ${event.error}`;
    default:
      return null;
  }
}

const REFRESH_ON = new Set(['connected', 'disconnected', 'battery', 'power']);

/** Subscribe to the event stream, reconnecting if the daemon restarts. */
export function connectEvents(onStateChange?: () => void): void {
  const open = (): void => {
    const source = new EventSource('/api/events');

    source.onmessage = (message: MessageEvent<string>) => {
      let event: LiveEvent;
      try {
        event = JSON.parse(message.data) as LiveEvent;
      } catch {
        return;
      }
      const line = describe(event);
      if (line) log(line);
      if (REFRESH_ON.has(event.kind)) onStateChange?.();
    };

    source.onerror = () => {
      source.close();
      window.setTimeout(open, 4000);
    };
  };

  open();
}
