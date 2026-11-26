// The PWA panel in its frame (RB-080-85): mounted as the panel frame mounts it; the panels window
// is played by spying on postMessage and dispatching `message` events from window.parent (the
// test window itself).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import en from '../../../locales/en.json';
import App from './App.vue';
import type { Answer, PageReport, Strings } from './model';

type Posted = { type?: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

function flatten(value: unknown, prefix = ''): Strings {
  if (typeof value === 'string') return { [prefix]: value };
  return Object.entries(value as Record<string, unknown>).reduce<Strings>(
    (all, [key, child]) => ({ ...all, ...flatten(child, prefix ? `${prefix}.${key}` : key) }),
    {},
  );
}
const STRINGS = { ...flatten(en), 'plugins.panel.call_failed': 'The plugin could not do that.' };

const PAGE: PageReport = {
  view: 1,
  views: [1, 2],
  url: 'http://localhost:8080/pwa-ok/',
  manifest: {
    url: 'http://localhost:8080/pwa-ok/manifest.json', status: 200, contentType: 'application/manifest+json', error: null,
    table: [['name', 'Demo'], ['display', 'standalone']], startUrl: '/', scope: '/', startUrlStatus: '200',
  },
  icons: [
    { index: 0, src: 'icons/192.png', url: 'http://localhost:8080/pwa-ok/icons/192.png', sizes: '192x192', type: 'image/png', purpose: ['any'], state: 'ok', status: 200, format: 'image/png', width: 192, height: 192, bytes: 412 },
    { index: 1, src: 'icons/512.png', url: null, sizes: '512x512', type: 'image/png', purpose: ['maskable'], state: 'unreachable', status: 404, format: null, width: null, height: null, bytes: null },
  ],
  serviceWorker: { secure: true, supported: true, registered: true, active: true, controlled: true, scope: '/', script: '/sw.js', state: 'activated', fetchHandler: true },
  offline: { tested: true, url: '/', cached: true },
  installability: { checked: true, errors: [], failure: null },
  themeColor: true,
  appleTouchIcon: null,
};

const ANSWER: Answer = {
  pages: [PAGE, { ...PAGE, view: 3, views: [3], url: 'http://localhost:8080/other/' }],
  summary: { error: 1, warning: 0, info: 0 },
  skipped: [{ view: 4, reason: 'Firefox' }],
  cut: false,
  findings: [
    {
      rule: 'manifest.icon_unreachable', severity: 'error', message: 'plugin.pwa-check.findings.unknown', params: {}, view: 1, url: null,
      path: 'pwa:icon', data: { panel: 'pwa', section: 'icons', anchor: 'icon-1' },
    },
  ],
};

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

async function answerCalls(tool: string, value: unknown, ok = true): Promise<void> {
  for (const message of posted.filter((m) => m.type === 'call' && (m.tool === tool || m.command === tool))) {
    await fromHost(ok ? { type: 'result', id: message.id, ok: true, value } : { type: 'result', id: message.id, ok: false, error: value });
  }
}

beforeEach(async () => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.pwa-check.', name: 'pwa panel' }).unmount;
  // RB-090-44: the panels window's message limit, 1 MiB here; the panel asks for answers of it.
  await fromHost({ type: 'settings', strings: STRINGS, settings: { 'plugins.panelMessageMaxBytes': 1024 * 1024 } });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the PWA panel', () => {
  it('checks again as its main action, sends to the findings panel from an icon button', () => {
    const again = $('#refresh') as HTMLButtonElement;
    expect(again.textContent).toBe('Check again');
    expect(again.hasAttribute('data-primary')).toBe(true);
    const report = $('#report') as HTMLButtonElement;
    expect(report.getAttribute('aria-label')).toBe('Send to the findings panel');
    expect(report.textContent).toBe('');
  });

  it('says it is checking, then shows the tabs with the findings first, the severity as a badge', async () => {
    expect($('#status')?.textContent).toBe('Checking…');
    expect(posted.find((m) => m.tool === 'check')?.args).toEqual({ budget: 1024 * 1024 - 5536 });
    await answerCalls('check', ANSWER);
    const tabs = [...document.querySelectorAll<HTMLElement>('#tabs [role="tab"]')];
    expect(tabs.map((tab) => tab.textContent?.trim())).toEqual(['Findings1', 'Manifest', 'Icons', 'Service worker', 'Installability']);
    // RB-090-48: the tab row scrolls sideways only, never a vertical scroll bar beside the tabs.
    const row = [...(document.querySelector('#tabs [role="tablist"]')?.classList ?? [])];
    expect(row).toContain('overflow-y-hidden');
    expect(row.filter((name) => /^overflow-(y-)?(auto|scroll)$/.test(name))).toEqual([]);
    const badge = $('#findings [data-severity]') as HTMLElement;
    expect(badge.textContent).toBe('Error');
    expect(badge.dataset.tone).toBe('error');
    expect($('#summary')?.textContent).toBe('1 error, 0 warnings, 0 notes');
    expect($('#notes')?.textContent).toBe('Not checked in view 4: Firefox');
    expect($('#views')?.textContent).toBe('Views: 1, 2');
  });

  it('jumps from a finding to its place in its tab', async () => {
    await answerCalls('check', ANSWER);
    ($('#findings [data-finding]') as HTMLButtonElement).click();
    await settle();
    const icons = [...document.querySelectorAll<HTMLElement>('#tabs [role="tab"]')][2] as HTMLElement;
    expect(icons.getAttribute('aria-selected')).toBe('true');
    expect(document.activeElement?.id).toBe('icon-1');
  });

  it('shows the manifest and the service worker as tables, the icons as cards with their state', async () => {
    await answerCalls('check', ANSWER);
    expect(document.querySelectorAll('#manifest table').length).toBe(2);
    expect($('#field-display')?.textContent).toBe('display');
    expect($('#sw table')?.textContent).toContain('Handles fetch');
    const cards = [...document.querySelectorAll<HTMLElement>('#icons [data-icon-card]')];
    expect(cards.map((card) => card.dataset.ok)).toEqual(['', undefined]);
    expect(cards[1]?.querySelector('[data-state]')?.getAttribute('data-tone')).toBe('error');
    expect(cards[1]?.querySelector('[data-safe-zone]')).not.toBeNull();
    expect(cards[0]?.querySelector('[data-safe-zone]')).toBeNull();
  });

  it('asks for the loaded icons and shows them from their data: URI', async () => {
    await answerCalls('check', ANSWER);
    expect(posted.filter((m) => m.tool === 'icon').map((m) => m.args)).toEqual([{ url: 'http://localhost:8080/pwa-ok/icons/192.png' }]);
    await answerCalls('icon', { dataUri: 'data:image/png;base64,iVBORw0KGgo=', tooLarge: false, bytes: 8 });
    expect($('#icon-0 img')?.getAttribute('src')).toBe('data:image/png;base64,iVBORw0KGgo=');
  });

  it('sends to the findings panel and says how many, and checks again on `checked` only when it did not send', async () => {
    await answerCalls('check', ANSWER);
    ($('#report') as HTMLButtonElement).click();
    expect(posted.some((m) => m.command === 'check')).toBe(true);
    const checks = posted.filter((m) => m.tool === 'check').length;
    await fromHost({ type: 'event', message: { type: 'checked' } });
    expect(posted.filter((m) => m.tool === 'check').length).toBe(checks);
    for (const message of posted.filter((m) => m.command === 'check')) await fromHost({ type: 'result', id: message.id, ok: true, value: { findings: 3 } });
    expect($('#status')?.textContent).toBe('3 findings sent to the findings panel.');
    expect($('#status')?.dataset.tone).toBe('ok');
    await fromHost({ type: 'event', message: { type: 'checked' } });
    expect(posted.filter((m) => m.tool === 'check').length).toBe(checks + 1);
  });

  // docs/design.md 5.6: a failure never degrades into the empty state.
  it('says a failed check in one sentence in the status line, not as "no page"', async () => {
    await answerCalls('check', 'plugins.error.busy', false);
    expect($('#status')?.dataset.tone).toBe('error');
    expect($('#status')?.textContent).toBe('The plugin could not do that.');
    expect($('#empty')).toBeNull();
  });

  it('says when no view shows a page', async () => {
    await answerCalls('check', { ...ANSWER, pages: [], findings: [], skipped: [] });
    expect($('#empty')?.textContent).toBe('No Chromium view shows a web page.');
  });

  it("takes the app's theme from its settings", async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'light' } });
    expect(document.documentElement.dataset.theme).toBe('light');
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });
});
