// A plugin panel's settings link (RB-096-11): `{ target: 'host', action: 'open_settings', key }`
// opens the settings window at the key - any key or area the settings know, the core's too - only
// within the person's click and not in a burst. Navigation only: nothing is written.
import { describe, expect, it, vi } from 'vitest';
import { LINK_INTERVAL_MS, OPAQUE_ORIGIN, PanelBridge, parseFromPanel, type PanelLinks, type PanelTarget } from './bridge';

function frame(): PanelTarget & { got: unknown[] } {
  const got: unknown[] = [];
  return {
    got,
    postMessage(message: unknown) {
      got.push(message);
    },
  };
}

function bridge(links: Partial<PanelLinks> = {}) {
  const opened: string[] = [];
  let clock = 10_000;
  const host = new PanelBridge(async () => ({ value: null }), undefined, {
    settings: vi.fn(async (key: string) => {
      opened.push(key);
      return true;
    }),
    gesture: () => true,
    now: () => clock,
    ...links,
  });
  const panel = frame();
  host.attach('api-client', panel);
  const post = (data: unknown) => host.receive({ source: panel, origin: OPAQUE_ORIGIN, data });
  const wait = (ms: number) => {
    clock += ms;
  };
  return { opened, panel, post, wait };
}

const open = (key: unknown, id?: number) => ({ target: 'host', action: 'open_settings', key, ...(id === undefined ? {} : { id }) });

describe("a plugin panel's settings link", () => {
  it('is a message of its own, with a key of the settings form', () => {
    expect(parseFromPanel(open('api-client.secrets', 4))).toEqual({ type: 'openSettings', key: 'api-client.secrets', id: 4 });
    expect(parseFromPanel(open('network'))).toEqual({ type: 'openSettings', key: 'network' });
    expect(parseFromPanel(open(7))).toBeNull();
    expect(parseFromPanel(open(''))).toBeNull();
    expect(parseFromPanel(open('../settings'))).toBeNull();
    expect(parseFromPanel(open('a b'))).toBeNull();
    expect(parseFromPanel(open(`a${'b'.repeat(200)}`))).toBeNull();
  });

  it("opens a key the person clicked, the core's keys too, and answers when asked", async () => {
    const { opened, panel, post, wait } = bridge();
    await post(open('api-client.benchRepeats', 7));
    wait(LINK_INTERVAL_MS);
    await post(open('mcp.port'));
    expect(opened).toEqual(['api-client.benchRepeats', 'mcp.port']);
    expect(panel.got).toEqual([{ type: 'result', id: 7, ok: true, value: { opened: true } }]);
  });

  it('opens nothing without a click, nor in a burst', async () => {
    const still = bridge({ gesture: () => false });
    await still.post(open('api-client.secrets', 1));
    expect(still.opened).toEqual([]);
    expect(still.panel.got).toEqual([{ type: 'result', id: 1, ok: false, error: 'links.error.gesture' }]);

    const burst = bridge();
    await burst.post(open('api-client.secrets', 1));
    await burst.post(open('api-client.secrets', 2));
    expect(burst.opened).toEqual(['api-client.secrets']);
    expect(burst.panel.got[1]).toEqual({ type: 'result', id: 2, ok: false, error: 'links.error.busy' });
  });

  it('opens nothing where the window gave no way to', async () => {
    const host = new PanelBridge(async () => ({ value: null }), undefined, { gesture: () => true });
    const panel = frame();
    host.attach('api-client', panel);
    await host.receive({ source: panel, origin: OPAQUE_ORIGIN, data: open('api-client.secrets', 3) });
    expect(panel.got).toEqual([{ type: 'result', id: 3, ok: true, value: { opened: false } }]);
  });
});
