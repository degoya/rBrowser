// A plugin panel's link (RB-080-92): `{ target: 'host', action: 'open_link', url }` opens through
// the core only for a web address, only within the person's click, and not in a burst.
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
    open: vi.fn(async (url: string) => {
      opened.push(url);
      return true;
    }),
    gesture: () => true,
    now: () => clock,
    ...links,
  });
  const panel = frame();
  host.attach('lighthouse', panel);
  const post = (data: unknown) => host.receive({ source: panel, origin: OPAQUE_ORIGIN, data });
  const wait = (ms: number) => {
    clock += ms;
  };
  return { opened, panel, post, wait };
}

const link = (url: string, id?: number) => ({ target: 'host', action: 'open_link', url, ...(id === undefined ? {} : { id }) });

describe("a plugin panel's link", () => {
  it('is a message of its own', () => {
    expect(parseFromPanel(link('https://web.dev/lcp', 3))).toEqual({ type: 'openLink', url: 'https://web.dev/lcp', id: 3 });
    expect(parseFromPanel({ target: 'host', action: 'open_link' })).toBeNull();
    expect(parseFromPanel({ target: 'host', action: 'open_file', url: 'https://x.test/' })).toBeNull();
  });

  it('opens a web address the person clicked and answers when asked', async () => {
    const { opened, panel, post } = bridge();
    await post(link('https://web.dev/lcp', 7));
    expect(opened).toEqual(['https://web.dev/lcp']);
    expect(panel.got).toEqual([{ type: 'result', id: 7, ok: true, value: { opened: true } }]);
  });

  it('refuses every other scheme and a disguised host', async () => {
    const { opened, panel, post } = bridge();
    await post(link('javascript:alert(1)', 1));
    await post(link('file:///C:/Windows/System32/calc.exe', 2));
    await post(link('https://user:pw@bank.example/', 3));
    expect(opened).toEqual([]);
    expect(panel.got).toEqual([
      { type: 'result', id: 1, ok: false, error: 'links.error.scheme' },
      { type: 'result', id: 2, ok: false, error: 'links.error.scheme' },
      { type: 'result', id: 3, ok: false, error: 'links.error.invalid' },
    ]);
  });

  it('opens nothing without a user gesture', async () => {
    const { opened, panel, post } = bridge({ gesture: () => false });
    await post(link('https://web.dev/lcp', 1));
    expect(opened).toEqual([]);
    expect(panel.got).toEqual([{ type: 'result', id: 1, ok: false, error: 'links.error.gesture' }]);
  });

  it('opens one link per interval and plugin', async () => {
    const { opened, panel, post, wait } = bridge();
    await post(link('https://web.dev/a'));
    await post(link('https://web.dev/b', 2));
    wait(LINK_INTERVAL_MS);
    await post(link('https://web.dev/c'));
    expect(opened).toEqual(['https://web.dev/a', 'https://web.dev/c']);
    expect(panel.got).toEqual([{ type: 'result', id: 2, ok: false, error: 'links.error.busy' }]);
  });

  it('opens nothing from a frame it did not attach', async () => {
    const { opened } = bridge();
    const host = new PanelBridge(async () => ({ value: null }), undefined, {
      open: async (url) => {
        opened.push(url);
        return true;
      },
      gesture: () => true,
    });
    expect(await host.receive({ source: frame(), origin: OPAQUE_ORIGIN, data: link('https://web.dev/') })).toBe(false);
    expect(opened).toEqual([]);
  });

  it('opens nothing when the window gave no way to', async () => {
    const host = new PanelBridge(async () => ({ value: null }));
    const panel = frame();
    host.attach('seo', panel);
    await host.receive({ source: panel, origin: OPAQUE_ORIGIN, data: link('https://web.dev/', 1) });
    expect(panel.got).toEqual([{ type: 'result', id: 1, ok: false, error: 'links.error.gesture' }]);
  });
});
