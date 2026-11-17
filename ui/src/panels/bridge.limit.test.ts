// RB-090-44: a panel's messages are held to `plugins.panelMessageMaxBytes` (4 MiB by default),
// which the core sends with every panel's settings - no longer to the page scripts' 64 KiB.
import { describe, expect, it } from 'vitest';
import { DEFAULT_MESSAGE_BYTES, LIMIT_REASON, MESSAGE_LIMIT_KEY, PanelBridge, messageLimitOf, type PanelTarget } from './bridge';
import type { PanelMessage, PanelRequest } from './protocol';

const MIB = 1024 * 1024;

function frame(): PanelTarget & { got: unknown[] } {
  const got: unknown[] = [];
  return { got, postMessage: (message: unknown) => got.push(message) };
}

/** A core whose settings name `limit` and whose calls answer `answer`. */
function core(answer: unknown, limit?: number) {
  const asked: PanelRequest[] = [];
  const ask = async (request: PanelRequest): Promise<PanelMessage> => {
    asked.push(request);
    if (request.op === 'settings') {
      return { strings: {}, settings: limit === undefined ? {} : { [MESSAGE_LIMIT_KEY]: limit } };
    }
    return { value: answer };
  };
  return { asked, ask };
}

const tooLarge = (id: number, limit: number) => ({
  type: 'result',
  id,
  ok: false,
  error: 'plugins.error.too_large',
  params: { limit: String(limit), reason: LIMIT_REASON, setting: MESSAGE_LIMIT_KEY },
});

describe('the panel message limit', () => {
  it('is 4 MiB by default: a call and its answer of 1 MiB go through', async () => {
    const big = 'x'.repeat(MIB);
    const { asked, ask } = core({ text: big });
    const bridge = new PanelBridge(ask);
    const panel = frame();
    bridge.attach('seo', panel);
    expect(bridge.messageLimit).toBe(DEFAULT_MESSAGE_BYTES);
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'call', id: 1, tool: 'get', args: { big } } });
    expect(asked).toHaveLength(1);
    expect(panel.got).toEqual([{ type: 'result', id: 1, ok: true, value: { text: big } }]);
  });

  it('follows the setting the core sends, and over it refuses naming the setting', async () => {
    const limit = 64 * 1024;
    const { asked, ask } = core({ text: 'x'.repeat(limit) }, limit);
    const bridge = new PanelBridge(ask);
    const panel = frame();
    bridge.attach('seo', panel);
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'settings' } });
    expect(bridge.messageLimit).toBe(limit);
    panel.got.length = 0;
    // Into the core: refused before it is asked.
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'call', id: 2, tool: 'get', args: 'x'.repeat(limit) } });
    expect(asked.filter((request) => request.op === 'call')).toEqual([]);
    // Out of the core: the answer is refused, the panel told why.
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'call', id: 3, tool: 'get' } });
    expect(panel.got).toEqual([tooLarge(2, limit), tooLarge(3, limit)]);
  });

  it('keeps the last limit when the settings name none', async () => {
    const { ask } = core(null);
    const bridge = new PanelBridge(ask);
    const panel = frame();
    bridge.attach('seo', panel);
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'settings' } });
    expect(bridge.messageLimit).toBe(DEFAULT_MESSAGE_BYTES);
  });

  it('is read from the settings held to its range', () => {
    expect(messageLimitOf({ [MESSAGE_LIMIT_KEY]: 2 * MIB })).toBe(2 * MIB);
    expect(messageLimitOf({ [MESSAGE_LIMIT_KEY]: 1 })).toBe(64 * 1024);
    expect(messageLimitOf({ [MESSAGE_LIMIT_KEY]: 64 * MIB })).toBe(16 * MIB);
    for (const none of [{}, { [MESSAGE_LIMIT_KEY]: '4194304' }, { [MESSAGE_LIMIT_KEY]: 1.5 }, { [MESSAGE_LIMIT_KEY]: null }]) {
      expect(messageLimitOf(none)).toBeUndefined();
    }
  });
});
