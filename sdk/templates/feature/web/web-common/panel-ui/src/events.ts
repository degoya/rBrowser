// The host's events for a Nuxt UI panel (RB-080-87): `{ type: 'event', message, view? }` from the
// panels window - a page script's `emit` (with its view), a component's `$post` to the panel, or
// the host's own (`{ host: 'waiting' }`, `hostNotice`). Only the panels window - `window.parent` -
// is heard. A module of its own beside `panel.ts`, so a panel that needs no events bundles none.

/** Calls `listener` with every event's message and its view (when the host names one); returns
 * what stops listening. */
export function onPanelEvent(listener: (message: unknown, view: number | undefined) => void, win: Window = window): () => void {
  const receive = (event: MessageEvent): void => {
    if (event.source !== win.parent) return;
    const data = event.data as Record<string, unknown> | null;
    if (!data || typeof data !== 'object' || data.type !== 'event') return;
    listener(data.message, typeof data.view === 'number' ? data.view : undefined);
  };
  win.addEventListener('message', receive);
  return () => win.removeEventListener('message', receive);
}
