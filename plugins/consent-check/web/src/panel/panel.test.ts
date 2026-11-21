// The consent check panel in its frame (RB-094-05): mounted as the panel frame mounts it; the
// panels window is played by spying on postMessage and dispatching `message` events from
// window.parent (the test window itself).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import { ANSWER, STRINGS as TEXTS, VIEW_ANSWER } from './fixtures';

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

/** Answers the calls of `name` (a tool or a command) not answered yet. */
async function reply(name: string, value: unknown, ok = true): Promise<void> {
  for (const message of posted.filter((m) => m.type === 'call' && (m.tool ?? m.command) === name && !answered.has(m.id ?? 0))) {
    answered.add(message.id ?? 0);
    await fromHost(ok ? { type: 'result', id: message.id, ok: true, value } : { type: 'result', id: message.id, ok: false, error: value });
  }
}

/** Runs the command as the panel's button does, the answer coming as the command's post. */
async function runCheck(answer: unknown): Promise<void> {
  ($('#run') as HTMLButtonElement).click();
  await settle();
  await fromHost({ type: 'event', message: { type: 'checked', panel: 'consent-check', answer } });
  await reply('check', { findings: 4, reported: true });
}

const tabButton = (index: number): HTMLElement => document.querySelectorAll<HTMLElement>('#tabs [role="tab"]')[index] as HTMLElement;
const openTab = async (index: number): Promise<void> => {
  tabButton(index).dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
  await settle();
};

beforeEach(async () => {
  posted = [];
  answered.clear();
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.consent-check.', name: 'consent check panel' }).unmount;
  await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the consent check panel', () => {
  it('starts no run by itself and always shows the notice', () => {
    expect(posted.some((m) => m.type === 'call')).toBe(false);
    expect($('#notice')?.textContent).toContain('Technical pre-check, no legal advice');
    expect($('#empty')?.textContent).toContain('Check the page');
  });

  it('runs the command and shows both phases and the findings', async () => {
    await runCheck(ANSWER);
    expect(posted.find((m) => m.command === 'check')).toBeDefined();
    expect($('#banner')?.textContent).toBe('"Reject all" was clicked: OneTrust');
    expect($('#mode')?.textContent).toContain('deleted afterwards');
    const tabs = [...document.querySelectorAll<HTMLElement>('#tabs [role="tab"]')];
    expect(tabs.map((one) => one.textContent?.trim())).toEqual(['Before consent', 'After "Reject all"', 'Findings4']);
    expect($('#before [data-host]')?.textContent).toContain('cdn.tracker.test');
    expect($('#before [data-cookie]')?.textContent).toContain('365 days');
    expect($('#status')?.textContent).toBe('4 findings sent to the findings panel.');
    await openTab(1);
    expect($('#after [data-diff="hostsAdded"]')?.textContent).toContain('pixel.ads.test (pixel)');
    await openTab(2);
    const first = $('#findings [data-finding]');
    expect(first?.textContent).toContain('pixel.ads.test');
  });

  it('names the caveats of a run in the view and has no step after reject', async () => {
    await runCheck(VIEW_ANSWER);
    const caveats = [...document.querySelectorAll<HTMLElement>('#caveats li')];
    expect(caveats).toHaveLength(3);
    const tabs = [...document.querySelectorAll<HTMLElement>('#tabs [role="tab"]')];
    expect(tabs.map((one) => one.textContent?.trim())).toEqual(['Before consent', 'Findings2']);
  });

  it('writes the export and says where', async () => {
    await runCheck(ANSWER);
    ($('#export') as HTMLButtonElement).click();
    await settle();
    expect(posted.find((m) => m.tool === 'check')?.args).toEqual({ export: true });
    await reply('check', { ...ANSWER, exported: '.rbrowser/out/consent-check/consent-check.json' });
    expect($('#status')?.textContent).toBe('Written to .rbrowser/out/consent-check/consent-check.json.');
  });

  it('says a failed run in one sentence', async () => {
    ($('#run') as HTMLButtonElement).click();
    await settle();
    await reply('check', 'plugins.error.busy', false);
    expect($('#status')?.textContent).toBe('The plugin could not do that.');
  });
});
