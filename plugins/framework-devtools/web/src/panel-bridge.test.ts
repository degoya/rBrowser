import { describe, expect, it } from 'vitest';
import { createPanelBridge } from './panel-bridge';

/** A frame whose parent is a fake panels window. */
function frame(): { win: Window; posted: unknown[]; deliver(message: unknown): void } {
  const posted: unknown[] = [];
  const listeners: ((event: MessageEvent) => void)[] = [];
  const parent = { postMessage: (message: unknown) => posted.push(message) };
  const win = {
    parent,
    addEventListener: (_type: string, listener: (event: MessageEvent) => void) => listeners.push(listener),
  } as unknown as Window;
  const deliver = (message: unknown): void => {
    for (const listener of listeners) listener({ source: parent, data: message } as unknown as MessageEvent);
  };
  return { win, posted, deliver };
}

describe('createPanelBridge', () => {
  it('hands the texts and the settings of the settings message over', () => {
    const { win, deliver } = frame();
    const bridge = createPanelBridge(win, 'plugin.x.');
    let told = 0;
    bridge.onSettings(() => told++);
    expect(bridge.text('panel.a')).toBe('panel.a');
    expect(bridge.setting('x.flag')).toBeUndefined();
    deliver({ type: 'settings', strings: { 'plugin.x.panel.a': 'A {n}' }, settings: { 'x.flag': false } });
    expect(told).toBe(1);
    expect(bridge.text('panel.a', { n: 2 })).toBe('A 2');
    expect(bridge.setting('x.flag')).toBe(false);
    deliver({ type: 'settings', strings: {} });
    expect(bridge.setting('x.flag')).toBeUndefined();
  });
});
