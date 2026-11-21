// The security header panel in its frame (RB-093-03): mounted as the panel frame mounts it; the
// panels window is played by spying on postMessage and dispatching `message` events from
// window.parent (the test window itself).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import { COMPARISON, STRINGS as TEXTS } from './fixtures';

type Posted = { type?: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };

const STRINGS = { ...TEXTS, 'plugins.panel.call_failed': 'The plugin could not do that.' };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;
const answered = new Set<number>();

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

/** Answers the calls of `name` not answered yet. */
async function reply(name: string, value: unknown, ok = true): Promise<void> {
  for (const message of posted.filter((m) => m.type === 'call' && (m.tool ?? m.command) === name && !answered.has(m.id ?? 0))) {
    answered.add(message.id ?? 0);
    await fromHost(ok ? { type: 'result', id: message.id, ok: true, value } : { type: 'result', id: message.id, ok: false, error: value });
  }
}

const lineKeys = (): (string | undefined)[] => [...document.querySelectorAll<HTMLElement>('#table [data-line]')].map((line) => line.dataset.line);

beforeEach(async () => {
  posted = [];
  answered.clear();
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.security-headers.', name: 'security header panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the security header panel', () => {
  it('compares at once and says so', () => {
    expect($('#status')?.textContent).toBe('Reading the page in each environment...');
    expect(posted.filter((m) => m.tool === 'compare')).toHaveLength(1);
  });

  it('draws a line per header and cookie, the differences marked with a word', async () => {
    await reply('compare', COMPARISON);
    expect(lineKeys()).toEqual(['content-security-policy', 'strict-transport-security', 'x-content-type-options', 'cookie:sid', 'cookie:pref']);
    const heads = [...document.querySelectorAll<HTMLElement>('#table thead th')].map((cell) => cell.textContent?.trim());
    expect(heads).toEqual(['Header', 'staging', 'prod']);
    const marked = [...document.querySelectorAll<HTMLElement>('#table [data-line]')].filter((line) => line.querySelector('.differs'));
    expect(marked.map((line) => line.dataset.line)).toEqual(['content-security-policy', 'x-content-type-options', 'cookie:sid']);
    expect(marked[0]?.textContent).toContain('Differs');
    expect($('#path')?.textContent).toBe('Page /shop');
    expect($('#status')?.textContent).toBe('3 differences');
  });

  it('shows only the differences when asked', async () => {
    await reply('compare', COMPARISON);
    ($('#only') as HTMLButtonElement).click();
    await settle();
    expect(lineKeys()).toEqual(['content-security-policy', 'x-content-type-options', 'cookie:sid']);
  });

  it("lists each environment's findings and why another could not be read", async () => {
    await reply('compare', COMPARISON);
    const staging = $('[data-environment="staging"]');
    expect(staging?.querySelectorAll('li')).toHaveLength(2);
    expect(staging?.querySelector('li')?.textContent).toContain("The Content-Security-Policy lets 'unsafe-inline' work for scripts.");
    expect($('[data-environment="local"]')?.textContent).toContain('The request failed: connection refused');
  });

  it('compares again on the command and says a failed call in one sentence', async () => {
    await reply('compare', COMPARISON);
    await fromHost({ type: 'event', message: { type: 'compare' } });
    expect(posted.filter((m) => m.tool === 'compare')).toHaveLength(2);
    await reply('compare', 'plugins.error.busy', false);
    expect($('#status')?.textContent).toBe('The plugin could not do that.');
  });

  // RB-094-21: "at least two environments (Settings > Project > Environments)" links there.
  it('links the failure for too few environments to them', async () => {
    await reply('compare', 'plugin.security-headers.errors.environments', false);
    ($('#status [data-testid="setting-link-environments"]') as HTMLElement).click();
    expect(posted).toContainEqual({ target: 'host', action: 'open_settings', key: 'environments' });
    await fromHost({ type: 'event', message: { type: 'compare' } });
    await reply('compare', 'plugins.error.busy', false);
    expect($('#status [data-testid="setting-link-environments"]')).toBeNull();
  });
});
