import { describe, expect, it } from 'vitest';
import { keyName, pressText, Recorder, type KeyPress } from './record';

function press(code: string, mods: Partial<KeyPress> = {}): KeyPress {
  return { code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...mods };
}
const leader = press('Space', { ctrlKey: true, shiftKey: true });

describe('recording a shortcut', () => {
  it('names keys by position, like rb-commands', () => {
    expect(keyName('KeyS')).toBe('S');
    expect(keyName('Digit1')).toBe('1');
    expect(keyName('F9')).toBe('F9');
    expect(keyName('Comma')).toBe(',');
    expect(keyName('ArrowLeft')).toBe('Left');
    expect(keyName('ShiftLeft')).toBeNull();
    expect(pressText(press('KeyT', { ctrlKey: true }))).toBe('Ctrl+T');
    expect(pressText(press('KeyQ', { ctrlKey: true, altKey: true }))).toBe('Ctrl+Alt+Q');
  });

  it('takes a direct combination in one press', () => {
    const recorder = new Recorder();
    expect(recorder.feed(press('ShiftLeft', { shiftKey: true }))).toBe('ignored');
    expect(recorder.feed(press('F9', { ctrlKey: true, shiftKey: true }))).toBe('done');
    expect(recorder.text()).toBe('Ctrl+Shift+F9');
  });

  it('takes the leader and two keys, or one key and Enter, and Escape cancels', () => {
    const two = new Recorder();
    expect(two.feed(leader)).toBe('more');
    expect(two.feed(press('KeyS'))).toBe('more');
    expect(two.feed(press('KeyS'))).toBe('done');
    expect(two.text()).toBe('Ctrl+Shift+Space S S');
    const one = new Recorder();
    one.feed(leader);
    one.feed(press('Comma'));
    expect(one.feed(press('Enter'))).toBe('done');
    expect(one.text()).toBe('Ctrl+Shift+Space ,');
    const cancelled = new Recorder();
    cancelled.feed(leader);
    expect(cancelled.feed(press('Escape'))).toBe('cancel');
    expect(cancelled.text()).toBe('');
  });
});
