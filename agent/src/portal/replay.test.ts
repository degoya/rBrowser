import { beforeEach, describe, expect, it } from 'vitest';
import type { ElementTarget, SyncEvent } from '../protocol';
import { replayOnDevice } from './replay';

const at = (path: string): ElementTarget => ({ element: { kind: 'css', path }, offset: { x: 0.5, y: 0.5 } });

function visible(element: HTMLElement): void {
  element.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, right: 100, bottom: 20, width: 100, height: 20, toJSON: () => ({}) });
}

describe('replay on a device', () => {
  beforeEach(() => {
    document.body.innerHTML = '<button id="go">Go</button><input id="name"><input id="ok" type="checkbox">';
    for (const element of document.querySelectorAll<HTMLElement>('button, input')) visible(element);
  });

  it('clicks by element.click()', () => {
    let clicks = 0;
    document.getElementById('go')!.addEventListener('click', () => {
      clicks += 1;
    });
    const click: SyncEvent = { type: 'click', button: 'left', detail: 1, modifiers: 0, target: at('#go') };
    expect(replayOnDevice(click, document)).toBe('done');
    expect(clicks).toBe(1);
  });

  it('answers not_found for an element the device does not have', () => {
    const click: SyncEvent = { type: 'click', button: 'left', detail: 1, modifiers: 0, target: at('#missing') };
    expect(replayOnDevice(click, document)).toBe('not_found');
  });

  it('sets the value of an input and fires input', () => {
    const input = document.getElementById('name') as HTMLInputElement;
    const seen: string[] = [];
    input.addEventListener('input', () => seen.push(input.value));
    const typed: SyncEvent = { type: 'input', inputType: 'insertText', data: 'a', value: 'Ada', target: at('#name') };
    expect(replayOnDevice(typed, document)).toBe('done');
    expect(input.value).toBe('Ada');
    expect(seen).toEqual(['Ada']);
  });

  it('applies a checkbox change', () => {
    const change: SyncEvent = { type: 'change', checked: true, target: at('#ok') };
    expect(replayOnDevice(change, document)).toBe('done');
    expect((document.getElementById('ok') as HTMLInputElement).checked).toBe(true);
  });

  it('cannot replay hover, keys or a submit by script', () => {
    const events: SyncEvent[] = [
      { type: 'hover', target: at('#go') },
      { type: 'keydown', key: 'a', code: 'KeyA', keyCode: 65, location: 0, repeat: false, modifiers: 0 },
      { type: 'submit', method: 'post', action: '/x', target: at('form') },
    ];
    for (const event of events) expect(replayOnDevice(event, document)).toBe('unsupported');
  });

  it('an input without a value is not replayable', () => {
    const typed: SyncEvent = { type: 'input', inputType: 'insertText', target: at('#name') };
    expect(replayOnDevice(typed, document)).toBe('unsupported');
  });
});
