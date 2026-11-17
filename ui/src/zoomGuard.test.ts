import { describe, expect, it } from 'vitest';
import { guardZoom } from './zoomGuard';

// The test DOM's WheelEvent drops the mouse fields: a mouse event of type `wheel` carries them.
function wheel(init: MouseEventInit): WheelEvent {
  return new MouseEvent('wheel', { bubbles: true, cancelable: true, ...init }) as WheelEvent;
}

function key(name: string, init: KeyboardEventInit = {}): KeyboardEvent {
  return new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true, ...init });
}

// RB-080-50: Chromium zooms a page on Ctrl+wheel and Ctrl+Plus/Minus/0 unless the page takes the
// event, and keeps the zoom per host - every page of the shell (toolbar, status bar, canvas
// board) grew together while the native views stayed. The shell's pages take these events.
describe('the zoom guard of the shell pages', () => {
  it('takes Ctrl+wheel and leaves a plain wheel alone', () => {
    const target = document.createElement('div');
    document.body.append(target);
    const stop = guardZoom(window);
    const zoom = wheel({ ctrlKey: true });
    target.dispatchEvent(zoom);
    expect(zoom.defaultPrevented).toBe(true);
    const scroll = wheel({});
    target.dispatchEvent(scroll);
    expect(scroll.defaultPrevented).toBe(false);
    stop();
    const after = wheel({ ctrlKey: true });
    target.dispatchEvent(after);
    expect(after.defaultPrevented).toBe(false);
    target.remove();
  });

  it('takes the zoom keys with Ctrl and no other key', () => {
    const stop = guardZoom(window);
    for (const name of ['+', '-', '=', '0', '_']) {
      const event = key(name, { ctrlKey: true });
      document.body.dispatchEvent(event);
      expect(event.defaultPrevented, name).toBe(true);
    }
    for (const event of [key('+'), key('a', { ctrlKey: true }), key('0', { ctrlKey: true, altKey: true })]) {
      document.body.dispatchEvent(event);
      expect(event.defaultPrevented).toBe(false);
    }
    stop();
  });
});
