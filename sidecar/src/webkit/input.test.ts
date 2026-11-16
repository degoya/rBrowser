import { describe, expect, it } from 'vitest';

import { FakePage } from './fake.ts';
import { InputState, dispatch, parseEvents } from './input.ts';

async function replay(events: unknown[], state = new InputState()) {
  const page = new FakePage();
  for (const event of parseEvents(events)) await dispatch(page, state, event);
  return page.calls;
}

describe('dispatch', () => {
  it('turns a click into move, press and release with the click count', async () => {
    expect(
      await replay([
        { type: 'mouse', phase: 'down', x: 5, y: 6, button: 'left', clicks: 2, modifiers: 0 },
        { type: 'mouse', phase: 'up', x: 5, y: 6, button: 'left', clicks: 2, modifiers: 0 },
      ]),
    ).toEqual(['mouse.move 5 6', 'mouse.down left 2', 'mouse.up left 2']);
  });

  it('moves only for a hover', async () => {
    expect(await replay([{ type: 'mouse', phase: 'move', x: 1, y: 2, button: 'none', clicks: 0, modifiers: 0 }])).toEqual([
      'mouse.move 1 2',
    ]);
  });

  it('holds the modifier keys a click asks for and releases them after', async () => {
    const state = new InputState();
    expect(
      await replay(
        [
          { type: 'mouse', phase: 'down', x: 0, y: 0, button: 'left', clicks: 1, modifiers: 2 | 8 },
          { type: 'mouse', phase: 'up', x: 0, y: 0, button: 'left', clicks: 1, modifiers: 2 | 8 },
          { type: 'mouse', phase: 'move', x: 1, y: 1, button: 'none', clicks: 0, modifiers: 0 },
        ],
        state,
      ),
    ).toEqual([
      'key.down Control',
      'key.down Shift',
      'mouse.move 0 0',
      'mouse.down left 1',
      'mouse.up left 1',
      'key.up Control',
      'key.up Shift',
      'mouse.move 1 1',
    ]);
    expect(state.held.size).toBe(0);
  });

  it('presses a modifier key once, whether by a key event or a click', async () => {
    expect(
      await replay([
        { type: 'key', phase: 'down', key: 'Shift', code: 'ShiftLeft', modifiers: 8 },
        { type: 'mouse', phase: 'down', x: 0, y: 0, button: 'left', clicks: 1, modifiers: 8 },
        { type: 'key', phase: 'down', key: 'A', code: 'KeyA', modifiers: 8 },
        { type: 'key', phase: 'up', key: 'A', code: 'KeyA', modifiers: 8 },
        { type: 'key', phase: 'up', key: 'Shift', code: 'ShiftLeft', modifiers: 0 },
        { type: 'key', phase: 'up', key: 'Shift', code: 'ShiftLeft', modifiers: 0 },
      ]),
    ).toEqual(['key.down Shift', 'mouse.move 0 0', 'mouse.down left 1', 'key.down A', 'key.up A', 'key.up Shift']);
  });

  it('turns the wheel at the point and inserts text', async () => {
    expect(
      await replay([
        { type: 'wheel', x: 3, y: 4, deltaX: 0, deltaY: 120 },
        { type: 'text', text: 'caf\u00e9' },
      ]),
    ).toEqual(['mouse.move 3 4', 'mouse.wheel 0 120', 'key.insertText caf\u00e9']);
  });
});

describe('parseEvents', () => {
  it('refuses more events than one call may carry', () => {
    const many = Array.from({ length: 65 }, () => ({ type: 'text', text: 'a' }));
    expect(() => parseEvents(many)).toThrow('at most 64');
  });

  it('refuses modifier bits beyond the four', () => {
    expect(() => parseEvents([{ type: 'key', phase: 'down', key: 'a', code: 'KeyA', modifiers: 16 }])).toThrow('modifiers');
  });
});
