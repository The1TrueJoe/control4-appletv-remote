// Small DOM helpers. Deliberately tiny — this UI has no framework and does not
// need one.

export function $<T extends HTMLElement = HTMLElement>(selector: string): T | null {
  return document.querySelector<T>(selector);
}

/** Query an element that must exist; throws early rather than failing later
 *  with a confusing null dereference. */
export function must<T extends HTMLElement = HTMLElement>(selector: string): T {
  const el = document.querySelector<T>(selector);
  if (!el) throw new Error(`missing element: ${selector}`);
  return el;
}

/** Escape for interpolation into HTML. Everything device-supplied goes through
 *  this: remote names come off the air and are not trusted. */
export function esc(value: unknown): string {
  const map: Record<string, string> = {
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  };
  return String(value ?? '').replace(/[&<>"']/g, (c) => map[c] ?? c);
}

interface FlashTarget extends HTMLElement {
  flashTimer?: number;
}

/** Transient status text next to a button. */
export function flash(selector: string, message: string, kind = ''): void {
  const el = $<FlashTarget>(selector);
  if (!el) return;
  el.textContent = message;
  el.className = `sub ${kind}`.trim();
  window.clearTimeout(el.flashTimer);
  el.flashTimer = window.setTimeout(() => { el.textContent = ''; }, 6000);
}

export function spinner(text: string): string {
  return `<span class="spin"></span> ${esc(text)}`;
}
