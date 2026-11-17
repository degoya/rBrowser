// RB-090-39: a panel's frame is a document of its own, so the shell's root font size never
// reaches it. The bridge gives every panel the shell's text size as `ui.textScale` with its
// settings, and sends them again when the size changes.
import { describe, expect, it } from 'vitest';
import { PanelBridge, TEXT_SCALE_KEY, type PanelTarget } from './bridge';
import type { PanelMessage, PanelRequest } from './protocol';

function frame(): PanelTarget & { got: unknown[] } {
  const got: unknown[] = [];
  return { got, postMessage: (message: unknown) => got.push(message) };
}

function core() {
  const asked: PanelRequest[] = [];
  const ask = async (request: PanelRequest): Promise<PanelMessage> => {
    asked.push(request);
    return { strings: { 'plugin.seo.title': 'SEO' }, settings: { 'ui.theme': 'dark', 'seo.depth': 2 } };
  };
  return { asked, ask };
}

const settingsOf = (got: unknown[]): unknown[] =>
  got.filter((message) => (message as { type?: string }).type === 'settings').map((message) => (message as { settings: unknown }).settings);

describe('the text size a panel gets', () => {
  it('is 100 until the shell names one, beside the core settings', async () => {
    const { ask } = core();
    const bridge = new PanelBridge(ask);
    const panel = frame();
    bridge.attach('seo', panel);
    await bridge.receive({ source: panel, origin: 'null', data: { type: 'settings' } });
    expect(settingsOf(panel.got)).toEqual([{ 'ui.theme': 'dark', 'seo.depth': 2, [TEXT_SCALE_KEY]: 100 }]);
  });

  it('is the shell text size, and a change sends every panel its settings again', async () => {
    const { asked, ask } = core();
    const bridge = new PanelBridge(ask);
    const seo = frame();
    const todo = frame();
    bridge.attach('seo', seo);
    bridge.attach('todo', todo);
    await bridge.setTextScale(150);
    expect(settingsOf(seo.got)).toEqual([{ 'ui.theme': 'dark', 'seo.depth': 2, [TEXT_SCALE_KEY]: 150 }]);
    expect(settingsOf(todo.got)).toHaveLength(1);
    await bridge.receive({ source: seo, origin: 'null', data: { type: 'settings' } });
    expect(settingsOf(seo.got)[1]).toMatchObject({ [TEXT_SCALE_KEY]: 150 });
    expect(asked.filter((request) => request.op === 'settings')).toHaveLength(3);
  });

  it('is held to 100..225, and the same size sends nothing again', async () => {
    const { asked, ask } = core();
    const bridge = new PanelBridge(ask);
    const panel = frame();
    bridge.attach('seo', panel);
    await bridge.setTextScale(400);
    expect(settingsOf(panel.got)).toEqual([expect.objectContaining({ [TEXT_SCALE_KEY]: 225 })]);
    await bridge.setTextScale(225);
    await bridge.setTextScale(Number.NaN);
    expect(settingsOf(panel.got).map((settings) => (settings as Record<string, unknown>)[TEXT_SCALE_KEY])).toEqual([225, 100]);
    expect(asked).toHaveLength(2);
  });
});
