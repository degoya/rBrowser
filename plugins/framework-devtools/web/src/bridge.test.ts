import { beforeEach, describe, expect, it } from 'vitest';
import { installRelay } from './bridge';
import { CHUNK_CHARS, FRAMEWORK_EVENT, inEvent, outEvent } from './protocol';

interface FakeBridge extends RbPluginBridge {
  emitted: unknown[];
  listeners: ((message: unknown) => void)[];
  stops: (() => void)[];
}

function fakeBridge(): FakeBridge {
  const bridge: FakeBridge = {
    id: 'framework-devtools',
    view: 4,
    maxMessageBytes: 65536,
    emitted: [],
    listeners: [],
    stops: [],
    send: () => Promise.resolve(null),
    emit(message) {
      bridge.emitted.push(message);
    },
    on(listener) {
      bridge.listeners.push(listener);
      return () => {
        bridge.listeners = bridge.listeners.filter((l) => l !== listener);
      };
    },
    onStop(listener) {
      bridge.stops.push(listener);
      return () => undefined;
    },
  };
  return bridge;
}

function dispatch(name: string, detail: unknown): void {
  document.dispatchEvent(new CustomEvent(name, { detail }));
}

function inbox(name: string): string[] {
  const seen: string[] = [];
  document.addEventListener(name, (event) => seen.push((event as CustomEvent<string>).detail));
  return seen;
}

beforeEach(() => {
  document.body.innerHTML = '<div id="app"></div>';
});

describe('installRelay', () => {
  it("relays a backend's message to the panel whole, a long one in chunks", () => {
    const bridge = fakeBridge();
    expect(installRelay(window, bridge)).toBe(true);
    dispatch(outEvent('react'), '{"t":"m","d":{"event":"operations"}}');
    expect(bridge.emitted).toEqual([{ framework: 'react', text: '{"t":"m","d":{"event":"operations"}}' }]);
    const long = 'x'.repeat(CHUNK_CHARS * 2 + 1);
    dispatch(outEvent('vue'), long);
    const parts = bridge.emitted.slice(1) as { framework: string; text: string; id: number; i: number; n: number }[];
    expect(parts).toHaveLength(3);
    expect(parts.map((p) => [p.framework, p.id, p.i, p.n])).toEqual([
      ['vue', 2, 0, 3],
      ['vue', 2, 1, 3],
      ['vue', 2, 2, 3],
    ]);
    expect(parts.map((p) => p.text).join('')).toBe(long);
    bridge.stops.forEach((stop) => stop());
  });

  it("relays the probe's report and ignores details that are no text", () => {
    const bridge = fakeBridge();
    installRelay(window, bridge);
    dispatch(FRAMEWORK_EVENT, '{"vue":{"hook":true,"devtools":true}}');
    dispatch(FRAMEWORK_EVENT, { vue: { hook: true, devtools: true } });
    dispatch(outEvent('react'), 42);
    expect(bridge.emitted).toEqual([{ framework: 'probe', text: '{"vue":{"hook":true,"devtools":true}}' }]);
    bridge.stops.forEach((stop) => stop());
  });

  it("hands the panel's posts to the backend and nothing else", () => {
    const bridge = fakeBridge();
    installRelay(window, bridge);
    const react = inbox(inEvent('react'));
    const vue = inbox(inEvent('vue'));
    for (const listener of bridge.listeners) {
      listener({ framework: 'react', envelope: { t: 'connect' } });
      listener({ framework: 'react', envelope: { t: 'm', d: { event: 'inspectElement', payload: 1 } } });
      listener({ framework: 'vue', envelope: { t: 'nope' } });
      listener('connect');
    }
    expect(react).toEqual(['{"t":"connect"}', '{"t":"m","d":{"event":"inspectElement","payload":1}}']);
    expect(vue).toEqual([]);
    bridge.stops.forEach((stop) => stop());
  });

  it('tells both backends to disconnect when stopped, then relays nothing', () => {
    const bridge = fakeBridge();
    installRelay(window, bridge);
    const react = inbox(inEvent('react'));
    const vue = inbox(inEvent('vue'));
    bridge.stops.forEach((stop) => stop());
    expect(react).toEqual(['{"t":"disconnect"}']);
    expect(vue).toEqual(['{"t":"disconnect"}']);
    expect(bridge.listeners).toEqual([]);
    dispatch(outEvent('react'), 'late');
    expect(bridge.emitted).toEqual([]);
  });

  it('survives an emit that throws and does nothing in a child frame', () => {
    const bridge = fakeBridge();
    bridge.emit = () => {
      throw new Error('plugins.error.too_large');
    };
    installRelay(window, bridge);
    expect(() => dispatch(outEvent('react'), 'x')).not.toThrow();
    bridge.stops.forEach((stop) => stop());
    const frame = { top: {} } as unknown as Window;
    expect(installRelay(frame, fakeBridge())).toBe(false);
  });
});
