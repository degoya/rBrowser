import { describe, expect, it } from 'vitest';
import { arrowCommand, type ArrowKey } from './focusKeys';

function key(over: Partial<ArrowKey> = {}): ArrowKey {
  return {
    key: 'ArrowRight',
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    target: document.body,
    ...over,
  };
}

describe('the focus-mode arrow keys', () => {
  it('step to the next and previous breakpoint in focus mode', () => {
    expect(arrowCommand(key(), 'focus')).toBe('layout.focus_next');
    expect(arrowCommand(key({ key: 'ArrowLeft' }), 'focus')).toBe('layout.focus_prev');
  });

  it('do nothing in grid mode, with a modifier or for other keys', () => {
    expect(arrowCommand(key(), 'grid')).toBeNull();
    expect(arrowCommand(key({ shiftKey: true }), 'focus')).toBeNull();
    expect(arrowCommand(key({ altKey: true }), 'focus')).toBeNull();
    expect(arrowCommand(key({ key: 'ArrowUp' }), 'focus')).toBeNull();
  });

  it('leave the address bar, selects and editable text alone', () => {
    const editable = document.createElement('div');
    editable.contentEditable = 'true';
    for (const target of [
      document.createElement('input'),
      document.createElement('textarea'),
      document.createElement('select'),
      editable,
    ]) {
      expect(arrowCommand(key({ target }), 'focus'), target.tagName).toBeNull();
    }
  });
});
