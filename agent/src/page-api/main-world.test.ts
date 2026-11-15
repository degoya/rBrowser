import { Window as HappyWindow } from 'happy-dom';
import { describe, expect, it, vi } from 'vitest';
import type { RbrowserPageApi } from '../../types/rbrowser';
import { BUS_IN_EVENT, BUS_OUT_EVENT, VIEW_EVENT, VIEWCHANGE_EVENT } from '../protocol';
import type { PageState } from '../protocol';
import { installPageApi } from './main-world';
import { STATE } from './test-state';

type Win = Window & typeof globalThis;

/** A fresh window per test: `window.__rbrowser` cannot be removed once defined. */
function fresh(): Win {
  return new HappyWindow({ url: 'https://app.test/' }) as unknown as Win;
}

function page(state: unknown = STATE, now: () => number = () => 0): Win {
  const win = fresh();
  installPageApi(win, state, now);
  return win;
}

function api(win: Win): RbrowserPageApi {
  return (win as unknown as { __rbrowser: RbrowserPageApi }).__rbrowser;
}

function outgoing(win: Win): string[] {
  const sent: string[] = [];
  win.document.addEventListener(BUS_OUT_EVENT, (event) => {
    sent.push((event as CustomEvent<string>).detail);
  });
  return sent;
}

function fire(win: Win, name: string, detail: unknown): void {
  win.document.dispatchEvent(new win.CustomEvent(name, { detail }));
}

describe('window.__rbrowser', () => {
  it('shows the view and is frozen', () => {
    const win = page();
    const rb = api(win);
    expect(rb).toMatchObject({ id: 2, label: 'Tablet', width: 768, height: 1024, index: 1 });
    expect(rb.engine).toBe('chromium');
    expect(rb.emulation).toEqual(STATE.info.emulation);
    expect(Object.isFrozen(rb)).toBe(true);
    expect(Object.isFrozen(rb.emulation)).toBe(true);
    expect(Object.isFrozen(rb.bus)).toBe(true);
    expect(() => {
      'use strict';
      (win as unknown as Record<string, unknown>).__rbrowser = {};
    }).toThrow(TypeError);
    expect(Object.keys(win)).not.toContain('__rbrowser');
  });

  it('is not there without a valid page state', () => {
    for (const state of [undefined, null, { info: STATE.info }, 'x']) {
      const win = fresh();
      expect(installPageApi(win, state)).toBe(false);
      expect(api(win)).toBeUndefined();
    }
  });

  it('is installed once', () => {
    const win = page();
    expect(installPageApi(win, STATE)).toBe(false);
  });

  it('follows a new breakpoint and fires rbrowser:viewchange', () => {
    const win = page();
    const seen: RbrowserPageApi[] = [];
    win.addEventListener(VIEWCHANGE_EVENT, (event) => {
      seen.push((event as CustomEvent<RbrowserPageApi>).detail);
    });
    const next: PageState = { ...STATE, info: { ...STATE.info, label: 'Mobile', width: 393 } };
    fire(win, VIEW_EVENT, JSON.stringify(next));
    expect(api(win).label).toBe('Mobile');
    expect(api(win).width).toBe(393);
    expect(seen).toHaveLength(1);
    expect(seen[0]).toBe(api(win));
    expect(Object.isFrozen(api(win))).toBe(true);
  });

  it('fires no viewchange for the same view, nor for a malformed one', () => {
    const win = page();
    const listener = vi.fn();
    win.addEventListener(VIEWCHANGE_EVENT, listener);
    fire(win, VIEW_EVENT, JSON.stringify(STATE));
    fire(win, VIEW_EVENT, '{"info":7');
    fire(win, VIEW_EVENT, { info: STATE.info });
    expect(listener).not.toHaveBeenCalled();
    expect(api(win).label).toBe('Tablet');
  });
});

describe('bus.send', () => {
  it('hands the message to the agent as JSON text', () => {
    const win = page();
    const sent = outgoing(win);
    expect(api(win).bus.send('canvas:pan', { x: 1 })).toBe(true);
    expect(sent).toEqual(['{"type":"canvas:pan","payload":{"x":1}}']);
  });

  it('throws for a bad type, a payload that is not JSON and one over the limit', () => {
    const win = page({ ...STATE, bus: { ...STATE.bus, maxPayloadBytes: 1024 } });
    const sent = outgoing(win);
    const bus = api(win).bus;
    expect(() => bus.send('', 1)).toThrow(TypeError);
    expect(() => bus.send('a b', 1)).toThrow(TypeError);
    expect(() => bus.send('x', () => 1)).toThrow(TypeError);
    expect(() => bus.send('x', 'y'.repeat(2000))).toThrow(RangeError);
    expect(sent).toEqual([]);
  });

  it('drops when the bus is off', () => {
    const win = page({ ...STATE, bus: { ...STATE.bus, enabled: false } });
    const sent = outgoing(win);
    expect(api(win).bus.send('x', 1)).toBe(false);
    expect(sent).toEqual([]);
  });

  it('throttles 1000 messages a second to the limit', () => {
    const now = 5000;
    const win = page(STATE, () => now);
    const sent = outgoing(win);
    let passed = 0;
    for (let i = 0; i < 1000; i += 1) if (api(win).bus.send('tick', i)) passed += 1;
    expect(passed).toBe(STATE.bus.maxPerSecond);
    expect(sent).toHaveLength(STATE.bus.maxPerSecond);
  });
});

describe('bus.on / bus.off', () => {
  it('calls the handlers of the type with payload and sender', () => {
    const win = page();
    const pan = vi.fn();
    const other = vi.fn();
    api(win).bus.on('canvas:pan', pan);
    api(win).bus.on('canvas:zoom', other);
    fire(win, BUS_IN_EVENT, JSON.stringify({ type: 'canvas:pan', payload: { x: 1 }, from: 1 }));
    expect(pan).toHaveBeenCalledWith({ x: 1 }, { type: 'canvas:pan', from: 1 });
    expect(other).not.toHaveBeenCalled();
  });

  it('stops after off, and one failing handler does not stop the others', () => {
    const win = page();
    const reported: unknown[] = [];
    (win as unknown as { reportError: (error: unknown) => void }).reportError = (error) =>
      reported.push(error);
    const failing = vi.fn(() => {
      throw new Error('boom');
    });
    const next = vi.fn();
    api(win).bus.on('x', failing);
    api(win).bus.on('x', next);
    fire(win, BUS_IN_EVENT, JSON.stringify({ type: 'x', payload: null }));
    expect(next).toHaveBeenCalledWith(null, { type: 'x', from: null });
    expect(reported).toHaveLength(1);
    api(win).bus.off('x', next);
    fire(win, BUS_IN_EVENT, JSON.stringify({ type: 'x', payload: 2 }));
    expect(next).toHaveBeenCalledTimes(1);
  });

  it('ignores malformed incoming messages and refuses a handler that is no function', () => {
    const win = page();
    const handler = vi.fn();
    api(win).bus.on('x', handler);
    fire(win, BUS_IN_EVENT, { type: 'x', payload: 1 });
    fire(win, BUS_IN_EVENT, '{"type":"x"');
    fire(win, BUS_IN_EVENT, JSON.stringify({ type: 'bad type', payload: 1 }));
    expect(handler).not.toHaveBeenCalled();
    expect(() => api(win).bus.on('x', 'nope' as unknown as () => void)).toThrow(TypeError);
  });
});
