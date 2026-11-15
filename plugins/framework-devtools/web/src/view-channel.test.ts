import { describe, expect, it } from 'vitest';
import type { PanelBridge } from './panel-bridge';
import { CHUNK_CHARS, relays } from './protocol';
import { openViewChannel, watchProbe } from './view-channel';
import { isFrameworkReport } from '../../../../agent/src/framework/report';

interface FakePanelBridge extends PanelBridge {
  calls: { tool: string; args: unknown }[];
  deliver(view: number | null, message: unknown): void;
}

function fakeBridge(): FakePanelBridge {
  const listeners = new Set<(view: number | null, message: unknown) => void>();
  const bridge: FakePanelBridge = {
    calls: [],
    call(tool, args) {
      bridge.calls.push({ tool, args });
      return Promise.resolve({});
    },
    text: (key) => key,
    setting: () => undefined,
    onSettings: () => () => undefined,
    onEvent(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    requestSettings: () => undefined,
    deliver(view, message) {
      for (const listener of listeners) listener(view, message);
    },
  };
  return bridge;
}

describe('openViewChannel', () => {
  it('hands the envelopes of its view and framework over, chunks put together', () => {
    const bridge = fakeBridge();
    const channel = openViewChannel(bridge, 3, 'react');
    const seen: unknown[] = [];
    channel.on((envelope) => seen.push(envelope));
    bridge.deliver(3, { framework: 'react', text: '{"t":"m","d":{"event":"operations","payload":[1]}}' });
    bridge.deliver(3, { framework: 'vue', text: '{"t":"m","d":1}' });
    bridge.deliver(4, { framework: 'react', text: '{"t":"m","d":2}' });
    bridge.deliver(3, { framework: 'react', text: 'not json' });
    bridge.deliver(3, { framework: 'react', text: '{"t":"nope"}' });
    const long = JSON.stringify({ t: 'm', d: 'y'.repeat(CHUNK_CHARS + 10) });
    for (const relay of relays('react', long, 9)) bridge.deliver(3, relay);
    expect(seen).toEqual([{ t: 'm', d: { event: 'operations', payload: [1] } }, JSON.parse(long)]);
    channel.close();
    bridge.deliver(3, { framework: 'react', text: '{"t":"connect"}' });
    expect(seen).toHaveLength(2);
  });

  it('posts through the send tool', async () => {
    const bridge = fakeBridge();
    const channel = openViewChannel(bridge, 2, 'vue');
    await channel.post({ t: 'connect' });
    expect(bridge.calls).toEqual([{ tool: 'send', args: { view: 2, framework: 'vue', envelope: { t: 'connect' } } }]);
  });
});

describe('watchProbe', () => {
  it('hands valid reports over with their view', () => {
    const bridge = fakeBridge();
    const seen: unknown[] = [];
    const stop = watchProbe(bridge, (view, report) => seen.push([view, report]));
    bridge.deliver(1, { framework: 'probe', text: '{"vue":{"hook":true,"devtools":true}}' });
    bridge.deliver(1, { framework: 'probe', text: '{"vue":{"hook":"x","devtools":true}}' });
    bridge.deliver(null, { framework: 'probe', text: '{}' });
    bridge.deliver(2, { framework: 'probe', text: '{}' });
    expect(seen).toEqual([[1, { vue: { hook: true, devtools: true } }], [2, {}]]);
    stop();
    expect(isFrameworkReport({ svelte: { hook: true, devtools: true } })).toBe(false);
  });

  it('checks a report with the agent\'s validator, the same bound on the version', () => {
    expect(isFrameworkReport({ vue: { hook: true, devtools: true, version: '3.5.13' } })).toBe(true);
    expect(isFrameworkReport({ vue: { hook: true, devtools: true, version: 'x'.repeat(64) } })).toBe(true);
    expect(isFrameworkReport({ vue: { hook: true, devtools: true, version: 'x'.repeat(65) } })).toBe(false);
    expect(isFrameworkReport({ react: { hook: true, devtools: true, version: 19 } })).toBe(false);
  });
});
