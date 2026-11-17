// The screenshot editor's keys anywhere in the panel (RB-080-01, RB-080-63): Ctrl+Z undo
// (Shift: redo), Ctrl+Y redo, Ctrl+S save, Ctrl+C on the canvas copy, Escape drops a draft or
// discards, a tool's letter chooses it, + and - zoom, 0 fits. Letters act only while the panel
// has the focus and never while typing - in a field, a menu or the colour popover, whose own
// keys (Escape closes it) go first.
import { toolForKey } from './editor';
import type { Session } from './session';

const TYPING = '[role="combobox"], [role="listbox"], [role="dialog"], [contenteditable="true"]';

export function isTyping(target: EventTarget | null): boolean {
  if (target instanceof HTMLInputElement || target instanceof HTMLSelectElement || target instanceof HTMLTextAreaElement) return true;
  return target instanceof Element && target.closest(TYPING) !== null;
}

/** The document's keydown listener of `session`; `canvas` is the drawing surface. */
export function keyHandler(session: Session, canvas: () => HTMLCanvasElement | null): (event: KeyboardEvent) => void {
  return (event) => {
    if (event.defaultPrevented) return;
    const target = event.target;
    const typing = isTyping(target);
    const ctrl = event.ctrlKey || event.metaKey;
    const key = event.key.toLowerCase();
    if (ctrl && !event.altKey && key === 'z' && !typing) {
      event.preventDefault();
      if (event.shiftKey) session.redo();
      else session.undo();
    } else if (ctrl && !event.altKey && key === 'y' && !typing) {
      event.preventDefault();
      session.redo();
    } else if (ctrl && !event.altKey && key === 's') {
      event.preventDefault();
      void session.save();
    } else if (ctrl && !event.altKey && key === 'c' && target !== null && target === canvas()) {
      event.preventDefault();
      void session.copy();
    } else if (event.key === 'Escape' && !typing) {
      session.escape();
    } else if (!ctrl && !event.altKey && !typing && !(target instanceof HTMLButtonElement && (key === ' ' || key === 'enter'))) {
      const chosen = event.key.length === 1 ? toolForKey(event.key) : null;
      if (chosen) {
        event.preventDefault();
        session.chooseByKey(chosen);
      } else if (event.key === '+' || event.key === '=') session.stepZoom(1);
      else if (event.key === '-') session.stepZoom(-1);
      else if (event.key === '0') session.fit();
    }
  };
}
