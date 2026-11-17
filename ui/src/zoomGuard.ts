// The shell's pages never zoom (RB-080-50). Chromium zooms a page on Ctrl+wheel (a pinch on a
// touchpad arrives as one) and on Ctrl+Plus/Minus/0 unless the page takes the event, and keeps
// the zoom per host: one turn of the wheel over the canvas board made the toolbar, the status bar
// and the board's frames grow together, while the native views kept their place. Every page of
// the shell takes these events (`mountShell`); the core holds the zoom at 0 besides
// (`cef_app/queries.rs`). The canvas board turns Ctrl+wheel into the canvas's own zoom. The
// shell still grows with Windows' text size (RB-090-26): through its root font size
// (`theme/textScale.ts`), never the page zoom, so a CSS px stays a DIP.

/** The keys Chromium zooms with while Ctrl is held. */
const ZOOM_KEYS: ReadonlySet<string> = new Set(['+', '-', '=', '_', '0']);

/** Takes the page-zoom gestures of `win` away from Chromium; returns how to stop. */
export function guardZoom(win: Window): () => void {
  const onWheel = (event: WheelEvent): void => {
    if (event.ctrlKey) {
      event.preventDefault();
    }
  };
  const onKey = (event: KeyboardEvent): void => {
    if (event.ctrlKey && !event.altKey && !event.metaKey && ZOOM_KEYS.has(event.key)) {
      event.preventDefault();
    }
  };
  // Not passive: a passive listener cannot take the wheel.
  win.addEventListener('wheel', onWheel, { passive: false, capture: true });
  win.addEventListener('keydown', onKey, { capture: true });
  return () => {
    win.removeEventListener('wheel', onWheel, { capture: true });
    win.removeEventListener('keydown', onKey, { capture: true });
  };
}
