// Replayed input in a WebKit view (RB-040-04): rb-engine's `InputEvent` (CSS px of the
// viewport) as Playwright's `page.mouse` and `page.keyboard`, which WebKit receives as real,
// trusted input. Modifier bits (Alt 1, Ctrl 2, Meta 4, Shift 8) become held modifier keys; the
// keys held are tracked per view, so a modifier pressed by a key event and one a click asks for
// are the same key.

import { finite, integer, invalid, object, text } from './params.ts';
import type { MouseButton, PageLike } from './types.ts';

/** rb-engine `InputEvent`, as serde writes it. */
export type InputEvent =
  | {
      type: 'mouse';
      phase: 'down' | 'up' | 'move';
      x: number;
      y: number;
      button: 'none' | MouseButton;
      clicks: number;
      modifiers: number;
    }
  | { type: 'wheel'; x: number; y: number; deltaX: number; deltaY: number }
  | { type: 'key'; phase: 'down' | 'up'; key: string; code: string; modifiers: number }
  | { type: 'text'; text: string };

/** The most events one `webkit.view.input` carries. */
export const MAX_EVENTS = 64;
/** The largest coordinate or delta accepted, in CSS px. */
const MAX_COORD = 1_000_000;

const MODIFIERS: ReadonlyArray<readonly [number, string]> = [
  [1, 'Alt'],
  [2, 'Control'],
  [4, 'Meta'],
  [8, 'Shift'],
];
const MODIFIER_KEYS = new Set(MODIFIERS.map(([, key]) => key));

/** The modifier keys held in one view. */
export class InputState {
  readonly held = new Set<string>();
}

function phase<T extends string>(value: unknown, allowed: readonly T[]): T {
  if (!(allowed as readonly unknown[]).includes(value)) {
    throw invalid(`phase must be one of ${allowed.join(', ')}`);
  }
  return value as T;
}

function coordinate(value: unknown, what: string): number {
  return finite(value, what, -MAX_COORD, MAX_COORD);
}

/** Checks one event. */
export function parseEvent(value: unknown): InputEvent {
  const e = object(value, 'event');
  switch (e.type) {
    case 'mouse': {
      const button = e.button;
      if (button !== 'none' && button !== 'left' && button !== 'middle' && button !== 'right') {
        throw invalid('button must be none, left, middle or right');
      }
      return {
        type: 'mouse',
        phase: phase(e.phase, ['down', 'up', 'move'] as const),
        x: coordinate(e.x, 'x'),
        y: coordinate(e.y, 'y'),
        button,
        clicks: integer(e.clicks, 'clicks', 0, 3),
        modifiers: integer(e.modifiers, 'modifiers', 0, 15),
      };
    }
    case 'wheel':
      return {
        type: 'wheel',
        x: coordinate(e.x, 'x'),
        y: coordinate(e.y, 'y'),
        deltaX: coordinate(e.deltaX, 'deltaX'),
        deltaY: coordinate(e.deltaY, 'deltaY'),
      };
    case 'key':
      return {
        type: 'key',
        phase: phase(e.phase, ['down', 'up'] as const),
        key: text(e.key, 'key', 64),
        code: text(e.code, 'code', 64),
        modifiers: integer(e.modifiers, 'modifiers', 0, 15),
      };
    case 'text':
      return { type: 'text', text: text(e.text, 'text', 64 * 1024) };
    default:
      throw invalid('event type must be mouse, wheel, key or text');
  }
}

/** Checks the event list of `webkit.view.input`. */
export function parseEvents(value: unknown): InputEvent[] {
  if (!Array.isArray(value) || value.length > MAX_EVENTS) {
    throw invalid(`events must be a list of at most ${MAX_EVENTS} events`);
  }
  return value.map(parseEvent);
}

/** Presses or releases modifier keys until exactly the ones in `bits` are held. */
async function holdModifiers(page: PageLike, state: InputState, bits: number): Promise<void> {
  for (const [bit, key] of MODIFIERS) {
    const wanted = (bits & bit) !== 0;
    if (wanted && !state.held.has(key)) {
      await page.keyboard.down(key);
      state.held.add(key);
    } else if (!wanted && state.held.has(key)) {
      await page.keyboard.up(key);
      state.held.delete(key);
    }
  }
}

/** Replays one event into `page`. */
export async function dispatch(page: PageLike, state: InputState, event: InputEvent): Promise<void> {
  switch (event.type) {
    case 'mouse': {
      await holdModifiers(page, state, event.modifiers);
      const clickCount = Math.max(1, event.clicks);
      if (event.phase !== 'up') {
        await page.mouse.move(event.x, event.y);
      }
      if (event.button === 'none' || event.phase === 'move') {
        return;
      }
      const options = { button: event.button, clickCount };
      await (event.phase === 'down' ? page.mouse.down(options) : page.mouse.up(options));
      return;
    }
    case 'wheel':
      await page.mouse.move(event.x, event.y);
      await page.mouse.wheel(event.deltaX, event.deltaY);
      return;
    case 'key':
      if (MODIFIER_KEYS.has(event.key)) {
        const down = event.phase === 'down';
        if (down !== state.held.has(event.key)) {
          await (down ? page.keyboard.down(event.key) : page.keyboard.up(event.key));
          if (down) state.held.add(event.key);
          else state.held.delete(event.key);
        }
        return;
      }
      await (event.phase === 'down' ? page.keyboard.down(event.key) : page.keyboard.up(event.key));
      return;
    case 'text':
      await page.keyboard.insertText(event.text);
  }
}
