import { describe, expect, it, vi } from 'vitest';
import { DEFAULT_TIMEOUT_MS } from '../ipc/transport';
import { DEFAULT_MESSAGE_BYTES, LIMIT_REASON, MAX_PENDING, MESSAGE_LIMIT_KEY, PANEL_CALL_TIMEOUT_MS, PanelBridge, codeOf, failureOf, parseFromPanel, type PanelTarget } from './bridge';
import type { PanelMessage, PanelRequest } from './protocol';

/** A frame that keeps what it was sent. */
function frame(): PanelTarget & { got: unknown[]; origins: string[] } {
  const got: unknown[] = [];
  const origins: string[] = [];
  return {
    got,
    origins,
    postMessage(message: unknown, origin: string) {
      got.push(message);
      origins.push(origin);
    },
  };
}

/** A core that answers calls with their request and settings with fixed texts. */
function core(fail = false) {
  const asked: PanelRequest[] = [];
  const ask = async (request: PanelRequest): Promise<PanelMessage> => {
    asked.push(request);
    if (fail) {
      throw new Error('plugin.hello-panel.error.nope n=1');
    }
    if (request.op === 'settings') {
      return { strings: { 'plugin.hello-panel.panels.main': 'Hello' }, settings: { 'hello-panel.greeting': 'Hi' } };
    }
    return { value: { echo: request } };
  };
  return { asked, ask };
}

describe('a panel message', () => {
  it('is one of the fixed list', () => {
    expect(parseFromPanel({ type: 'settings' })).toEqual({ type: 'settings' });
    expect(parseFromPanel({ type: 'call', id: 1, tool: 'greet', args: { a: 1 } })).toEqual({
      type: 'call',
      id: 1,
      tool: 'greet',
      args: { a: 1 },
    });
    expect(parseFromPanel({ type: 'call', id: 2, command: 'say_hello' })).toMatchObject({ command: 'say_hello' });
    expect(parseFromPanel({ type: 'dirty', dirty: true })).toEqual({ type: 'dirty', dirty: true });
    expect(parseFromPanel({ type: 'dirty', dirty: false, extra: 1 })).toEqual({ type: 'dirty', dirty: false });
    expect(parseFromPanel({ type: 'keep', extra: 1 })).toEqual({ type: 'keep' });
    for (const bad of [
      { type: 'dirty' },
      { type: 'dirty', dirty: 'yes' },
      null,
      'call',
      [],
      { type: 'install' },
      { type: 'call', tool: 'greet' },
      { type: 'call', id: 1.5, tool: 'greet' },
      { type: 'call', id: 1 },
      { type: 'call', id: 1, tool: 'a', command: 'b' },
      { type: 'call', id: 1, tool: 'Greet' },
      { type: 'call', id: 1, tool: '../x' },
    ]) {
      expect(parseFromPanel(bad)).toBeNull();
    }
  });
});

describe('a failed call', () => {
  it('answers the code the core named, else host_failed', () => {
    expect(codeOf(new Error('plugin.hello-panel.error.nope n=1'))).toBe('plugin.hello-panel.error.nope');
    expect(codeOf(new Error('plugins.error.busy'))).toBe('plugins.error.busy');
    expect(codeOf(new Error('no answer to pluginPanel in 10000 ms'))).toBe('plugins.error.host_failed');
    expect(codeOf('weird')).toBe('plugins.error.host_failed');
  });
});

describe('the panel bridge', () => {
  it('answers a call of an attached panel with its own plugin', async () => {
    const { asked, ask } = core();
    const bridge = new PanelBridge(ask);
    const panel = frame();
    bridge.attach('hello-panel', panel);
    const handled = await bridge.receive({
      source: panel,
      origin: 'null',
      data: { type: 'call', id: 4, tool: 'greet', args: { name: 'x' } },
    });
    expect(handled).toBe(true);
    expect(asked).toEqual([{ op: 'call', plugin: 'hello-panel', tool: 'greet', args: { name: 'x' } }]);
    expect(panel.got).toEqual([
      { type: 'result', id: 4, ok: true, value: { echo: asked[0] } },
    ]);
    expect(panel.origins).toEqual(['*']);
  });

  it('ignores a sender it did not attach and an origin that is not opaque', async () => {
    const { asked, ask } = core();
    const bridge = new PanelBridge(ask);
    const panel = frame();
    bridge.attach('hello-panel', panel);
    const data = { type: 'call', id: 1, tool: 'greet' };
    expect(await bridge.receive({ source: frame(), origin: 'null', data })).toBe(false);
    expect(await bridge.receive({ source: window, origin: 'null', data })).toBe(false);
    expect(await bridge.receive({ source: panel, origin: 'rbrowser://ui', data })).toBe(false);
    expect(asked).toEqual([]);
    expect(panel.got).toEqual([]);
  });

  it('cannot reach another plugin: the plugin is the frame it came from', async () => {
    const { asked, ask } = core();
    const bridge = new PanelBridge(ask);
    const panel = frame();
    bridge.attach('hello-panel', panel);
    await bridge.receive({
      source: panel,
      origin: 'null',
      data: { type: 'call', id: 1, tool: 'greet', plugin: 'other' },
    });
    expect(asked[0]).toMatchObject({ plugin: 'hello-panel' });
  });

  it('refuses a message over the limit and drops what is not on the list', async () => {
    const { asked, ask } = core();
    const bridge = new PanelBridge(ask);
    const panel = frame();
    bridge.attach('hello-panel', panel);
    await bridge.receive({
      source: panel,
      origin: 'null',
      data: { type: 'call', id: 9, tool: 'greet', args: 'x'.repeat(DEFAULT_MESSAGE_BYTES) },
    });
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'eval', code: '1' } });
    expect(asked).toEqual([]);
    expect(panel.got).toEqual([
      {
        type: 'result',
        id: 9,
        ok: false,
        error: 'plugins.error.too_large',
        params: { limit: String(DEFAULT_MESSAGE_BYTES), reason: LIMIT_REASON, setting: MESSAGE_LIMIT_KEY },
      },
    ]);
  });

  it('answers a failed call with its code', async () => {
    const { ask } = core(true);
    const bridge = new PanelBridge(ask);
    const panel = frame();
    bridge.attach('hello-panel', panel);
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'call', id: 2, command: 'say' } });
    expect(panel.got).toEqual([{ type: 'result', id: 2, ok: false, error: 'plugin.hello-panel.error.nope', params: { n: '1' } }]);
  });

  // RB-080-90: the panels window gave a call up after the shell's 10 s and the panel got
  // `plugins.error.host_failed` without a reason, while the call still waited behind a check run
  // of the same plugin; and the reason the core named was cut off.
  it('gives a call the plugin\'s time, not the shell\'s default', async () => {
    const options: unknown[] = [];
    const bridge = new PanelBridge(async (request, sent) => {
      options.push(sent);
      return request.op === 'settings' ? { strings: {}, settings: {} } : { value: 1 };
    });
    const panel = frame();
    bridge.attach('fonts', panel);
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'call', id: 1, tool: 'list' } });
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'settings' } });
    expect(options).toEqual([{ timeoutMs: PANEL_CALL_TIMEOUT_MS }, undefined]);
    expect(PANEL_CALL_TIMEOUT_MS).toBeGreaterThan(DEFAULT_TIMEOUT_MS);
  });

  it('reads the code and the parameters of a failure, a value with spaces included', () => {
    expect(failureOf(new Error('plugin.hello-panel.error.nope n=1'))).toEqual({ code: 'plugin.hello-panel.error.nope', params: { n: '1' } });
    expect(failureOf(new Error('plugins.error.host_failed message=TypeError: x is not a function reason=exception'))).toEqual({
      code: 'plugins.error.host_failed',
      params: { message: 'TypeError: x is not a function', reason: 'exception' },
    });
    expect(failureOf(new Error('plugins.error.busy'))).toEqual({ code: 'plugins.error.busy', params: {} });
    expect(failureOf(new Error('no answer to pluginPanel in 600000 ms'))).toEqual({
      code: 'plugins.error.host_failed',
      params: { reason: 'no_answer' },
    });
    expect(failureOf('weird')).toEqual({ code: 'plugins.error.host_failed', params: {} });
  });

  it('logs a call the panels window gave up, once a minute per plugin and call', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    let now = 1_000_000;
    const bridge = new PanelBridge(
      async () => {
        throw new Error('no answer to pluginPanel in 600000 ms');
      },
      undefined,
      { now: () => now },
    );
    const panel = frame();
    bridge.attach('fonts', panel);
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'call', id: 1, tool: 'list' } });
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'call', id: 2, tool: 'list' } });
    expect(panel.got).toEqual([
      { type: 'result', id: 1, ok: false, error: 'plugins.error.host_failed', params: { reason: 'no_answer' } },
      { type: 'result', id: 2, ok: false, error: 'plugins.error.host_failed', params: { reason: 'no_answer' } },
    ]);
    expect(warn).toHaveBeenCalledTimes(1);
    expect(String(warn.mock.calls[0]?.[0])).toBe('plugin panel of fonts: list failed: plugins.error.host_failed reason=no_answer');
    now += 60_000;
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'call', id: 3, tool: 'list' } });
    expect(warn).toHaveBeenCalledTimes(2);
    warn.mockRestore();
  });

  it('refuses calls beyond the pending limit as busy', async () => {
    const release: Array<() => void> = [];
    const bridge = new PanelBridge(
      () => new Promise<PanelMessage>((resolve) => release.push(() => resolve({ value: 1 }))),
    );
    const panel = frame();
    bridge.attach('hello-panel', panel);
    const calls = Array.from({ length: MAX_PENDING + 1 }, (_, id) =>
      bridge.receive({ source: panel, origin: 'null', data: { type: 'call', id, tool: 'greet' } }),
    );
    await Promise.resolve();
    expect(panel.got).toEqual([{ type: 'result', id: MAX_PENDING, ok: false, error: 'plugins.error.busy' }]);
    release.forEach((done) => done());
    await Promise.all(calls);
    expect(panel.got).toHaveLength(MAX_PENDING + 1);
  });

  it('hands settings and texts to the panel that asks and to every panel on a change', async () => {
    const { ask } = core();
    const bridge = new PanelBridge(ask);
    const one = frame();
    const two = frame();
    bridge.attach('hello-panel', one);
    bridge.attach('other', two);
    await bridge.receive({ source: one, origin: 'null', data: { type: 'settings' } });
    expect(one.got).toEqual([
      {
        type: 'settings',
        strings: { 'plugin.hello-panel.panels.main': 'Hello' },
        settings: { 'hello-panel.greeting': 'Hi' },
      },
    ]);
    await bridge.refreshSettings();
    expect(one.got).toHaveLength(2);
    expect(two.got).toHaveLength(1);
  });

  // Review of RB-080-59: a panel says whether it holds unsaved work (the screenshot editor), so
  // the window keeps its frame alive; it learns nothing and is granted nothing by it.
  it('hands a panel\'s unsaved state to the window, for that frame only', async () => {
    const seen: Array<[unknown, boolean]> = [];
    const { ask, asked } = core();
    const bridge = new PanelBridge(ask, (target, dirty) => seen.push([target, dirty]));
    const one = frame();
    bridge.attach('hello-panel', one);
    await bridge.receive({ source: one, origin: 'null', data: { type: 'dirty', dirty: true } });
    await bridge.receive({ source: frame(), origin: 'null', data: { type: 'dirty', dirty: true } });
    await bridge.receive({ source: one, origin: 'https://evil.test', data: { type: 'dirty', dirty: false } });
    expect(seen).toEqual([[one, true]]);
    expect(asked).toEqual([]);
    expect(one.got).toEqual([]);
  });

  // RB-110-16: a panel that is slow to fill (the todo panel's search) asks to keep its frame; it
  // learns nothing and is granted nothing by it.
  it('hands a panel\'s wish to be kept to the window, for that frame only', async () => {
    const kept: unknown[] = [];
    const { ask, asked } = core();
    const bridge = new PanelBridge(ask, undefined, {}, (target) => kept.push(target));
    const one = frame();
    bridge.attach('hello-panel', one);
    await bridge.receive({ source: one, origin: 'null', data: { type: 'keep' } });
    await bridge.receive({ source: frame(), origin: 'null', data: { type: 'keep' } });
    await bridge.receive({ source: one, origin: 'https://evil.test', data: { type: 'keep' } });
    expect(kept).toEqual([one]);
    expect(asked).toEqual([]);
    expect(one.got).toEqual([]);
  });

  // RB-080-59: a failed answer left the panel without texts and the log without a trace.
  it('logs a settings request the core did not answer, with its code only', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const bridge = new PanelBridge(core(true).ask);
    const one = frame();
    bridge.attach('hello-panel', one);
    await bridge.receive({ source: one, origin: 'null', data: { type: 'settings' } });
    expect(one.got).toEqual([]);
    expect(warn).toHaveBeenCalledTimes(1);
    const line = String(warn.mock.calls[0]?.[0]);
    expect(line).toContain('hello-panel');
    expect(line).toContain('plugin.hello-panel.error.nope');
    expect(line).not.toContain('n=1');
    warn.mockRestore();
  });

  it('delivers an event to the panels of its plugin only, and none after detaching', () => {
    const bridge = new PanelBridge(core().ask);
    const mine = frame();
    const other = frame();
    const detach = bridge.attach('hello-panel', mine);
    bridge.attach('other', other);
    bridge.deliver({ plugin: 'hello-panel', view: 2, message: { text: 'hi' } });
    bridge.deliver({ plugin: 'hello-panel', message: 'from the component' });
    expect(mine.got).toEqual([
      { type: 'event', message: { text: 'hi' }, view: 2 },
      { type: 'event', message: 'from the component' },
    ]);
    expect(other.got).toEqual([]);
    detach();
    bridge.deliver({ plugin: 'hello-panel', message: 'late' });
    expect(mine.got).toHaveLength(2);
  });
});
