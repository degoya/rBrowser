import { Window as HappyWindow } from 'happy-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { RbrowserPageApi } from '../../types/rbrowser';
import { Agent } from '../agent';
import type { AgentMessage, CommandResult, PageState } from '../protocol';
import { BUS_IN_EVENT, BUS_OUT_EVENT, VIEW_EVENT, VIEWCHANGE_EVENT } from '../protocol';
import { installPageApi } from './main-world';
import { STATE } from './test-state';

type Win = Window & typeof globalThis;

interface View {
  id: number;
  win: Win;
  agent: Agent;
  sent: AgentMessage[];
  api: () => RbrowserPageApi;
}

function stateOf(id: number, bus: Partial<PageState['bus']> = {}): PageState {
  return { info: { ...STATE.info, id }, bus: { ...STATE.bus, ...bus } };
}

/** A view as the core sets it up: the main-world API, the agent, then the `page` command. */
function view(id: number, url = 'https://app.test/canvas', bus: Partial<PageState['bus']> = {}): View {
  const win = new HappyWindow({ url }) as unknown as Win;
  installPageApi(win, stateOf(id, bus));
  const sent: AgentMessage[] = [];
  const agent = new Agent({ win, send: (payload) => sent.push(JSON.parse(payload) as AgentMessage) });
  agent.start();
  expect(agent.command({ cmd: 'page', page: stateOf(id, bus) })).toEqual({ status: 'done' });
  const api = (): RbrowserPageApi => (win as unknown as { __rbrowser: RbrowserPageApi }).__rbrowser;
  return { id, win, agent, sent, api };
}

function busMessages(target: View): Extract<AgentMessage, { kind: 'bus' }>[] {
  return target.sent.flatMap((message) => (message.kind === 'bus' ? [message] : []));
}

/** The core's relay, as `crates/rbrowser/src/sync_agent/bus.rs` does it: to every other view. */
function relay(views: View[]): CommandResult[] {
  const results: CommandResult[] = [];
  for (const from of views) {
    for (const message of busMessages(from)) {
      for (const to of views) {
        if (to === from) continue;
        const { type, payload, origin } = message;
        results.push(to.agent.command({ cmd: 'bus', from: from.id, origin, type, payload }));
      }
    }
    from.sent.length = 0;
  }
  return results;
}

function forge(target: View, detail: unknown, on: EventTarget = target.win.document): void {
  on.dispatchEvent(new target.win.CustomEvent(BUS_OUT_EVENT, { detail, bubbles: true }));
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(1_762_790_400_000);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('the bus through the agents', () => {
  it('sends a page message to the core with the page origin', () => {
    const a = view(1);
    a.api().bus.send('canvas:pan', { x: 1 });
    expect(busMessages(a)).toEqual([
      {
        kind: 'bus', v: 1, top: true, time: 1_762_790_400_000, origin: 'https://app.test',
        type: 'canvas:pan', payload: { x: 1 },
      },
    ]);
  });

  it('calls the handlers in B and C, not in A', () => {
    const views = [view(1), view(2), view(3)];
    const [a, b, c] = views as [View, View, View];
    const handlers = views.map((target) => {
      const handler = vi.fn();
      target.api().bus.on('canvas:pan', handler);
      return handler;
    });
    a.api().bus.send('canvas:pan', { x: 1 });
    expect(relay(views)).toEqual([{ status: 'done' }, { status: 'done' }]);
    expect(handlers[0]).not.toHaveBeenCalled();
    expect(handlers[1]).toHaveBeenCalledWith({ x: 1 }, { type: 'canvas:pan', from: 1 });
    expect(handlers[2]).toHaveBeenCalledWith({ x: 1 }, { type: 'canvas:pan', from: 1 });
    expect([b.sent, c.sent].flat().filter((message) => message.kind === 'bus')).toEqual([]);
  });

  it('delivers only to a page of the same origin, and only while the bus is on', () => {
    const other = view(2, 'https://elsewhere.test/');
    const off = view(3, 'https://app.test/', { enabled: false });
    const message = { cmd: 'bus', from: 1, origin: 'https://app.test', type: 'x', payload: 1 };
    expect(other.agent.command(message)).toEqual({ status: 'error', message: 'other origin' });
    expect(off.agent.command(message)).toEqual({ status: 'error', message: 'bus off' });
    const core = { cmd: 'bus', type: 'x', payload: 1 };
    expect(other.agent.command(core)).toEqual({ status: 'done' });
  });

  it('refuses malformed bus and page commands', () => {
    const a = view(1);
    expect(a.agent.command({ cmd: 'bus', type: 'a b', payload: 1 })).toEqual({
      status: 'error',
      message: 'invalid bus message',
    });
    expect(a.agent.command({ cmd: 'bus', type: 'x', from: 'me', payload: 1 })).toEqual({
      status: 'error',
      message: 'invalid bus message',
    });
    expect(a.agent.command({ cmd: 'page', page: { info: STATE.info } })).toEqual({
      status: 'error',
      message: 'invalid page',
    });
  });

  it('hands a new page state to the main world, which fires rbrowser:viewchange', () => {
    const a = view(1);
    const changes = vi.fn();
    a.win.addEventListener(VIEWCHANGE_EVENT, changes);
    const next = stateOf(1);
    next.info = { ...next.info, label: 'Mobile', width: 393 };
    expect(a.agent.command({ cmd: 'page', page: next })).toEqual({ status: 'done' });
    expect(changes).toHaveBeenCalledTimes(1);
    expect(a.api().width).toBe(393);
  });
});

describe('bridge hardening (forged events)', () => {
  it('relays nothing but well-formed bus messages from the document', () => {
    const a = view(1);
    forge(a, { type: 'x', payload: 1 });
    forge(a, '{"type":"x","payload":1');
    forge(a, JSON.stringify({ type: 'x', payload: 1, cmd: 'replay', ms: 10_000 }));
    forge(a, JSON.stringify({ cmd: 'configure', config: { active: true } }));
    forge(a, JSON.stringify({ type: 'bad type', payload: 1 }));
    forge(a, JSON.stringify({ type: 'x', payload: 1 }), a.win.document.body);
    for (const name of [VIEW_EVENT, BUS_IN_EVENT, '__rbrowser:command', '__rbAgent']) {
      a.win.document.dispatchEvent(
        new a.win.CustomEvent(name, { detail: JSON.stringify({ cmd: 'configure' }) }),
      );
    }
    expect(a.sent.filter((message) => message.kind !== 'ready')).toEqual([]);
    forge(a, JSON.stringify({ type: 'x', payload: 1 }));
    expect(busMessages(a)).toHaveLength(1);
  });

  it('keeps its own switch when the page forges a page state', () => {
    const a = view(1, 'https://app.test/', { enabled: false });
    const forged = stateOf(1, { enabled: true, maxPerSecond: 1000 });
    a.win.document.dispatchEvent(
      new a.win.CustomEvent(VIEW_EVENT, { detail: JSON.stringify(forged) }),
    );
    expect(a.api().bus.send('x', 1)).toBe(true);
    expect(busMessages(a)).toEqual([]);
  });

  it('holds its own size and rate limits against raw events', () => {
    const a = view(1, 'https://app.test/', { maxPayloadBytes: 1024 });
    forge(a, JSON.stringify({ type: 'x', payload: 'y'.repeat(2000) }));
    expect(busMessages(a)).toEqual([]);
    for (let i = 0; i < 1000; i += 1) forge(a, JSON.stringify({ type: 'tick', payload: i }));
    expect(busMessages(a)).toHaveLength(STATE.bus.maxPerSecond);
    vi.advanceTimersByTime(1000);
    forge(a, JSON.stringify({ type: 'tick', payload: 0 }));
    expect(busMessages(a)).toHaveLength(STATE.bus.maxPerSecond + 1);
  });

  it('relays nothing before the core sent the page state', () => {
    const win = new HappyWindow({ url: 'https://app.test/' }) as unknown as Win;
    const sent: AgentMessage[] = [];
    const agent = new Agent({ win, send: (payload) => sent.push(JSON.parse(payload) as AgentMessage) });
    agent.start();
    win.document.dispatchEvent(
      new win.CustomEvent(BUS_OUT_EVENT, { detail: JSON.stringify({ type: 'x', payload: 1 }) }),
    );
    expect(sent.map((message) => message.kind)).toEqual(['ready']);
  });
});
