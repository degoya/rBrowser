// The panel in a window of its own: the bridge messages it sends and how it renders the
// plugin's answers. The panels window is played by spying on postMessage and dispatching
// `message` events from window.parent (the test window itself). On Nuxt UI since RB-080-83.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import type { LighthouseStatus, LighthouseSummary } from './panel/model';

type Posted = { type?: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown>; action?: string; url?: string };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const STRINGS: Record<string, string> = {
  'plugin.lighthouse.panels.lighthouse': 'Lighthouse',
  'plugin.lighthouse.panel.run': 'Run Lighthouse',
  'plugin.lighthouse.panel.cancel': 'Cancel',
  'plugin.lighthouse.panel.breakpoint': 'Breakpoint',
  'plugin.lighthouse.panel.categories.performance': 'Performance',
  'plugin.lighthouse.panel.categories.seo': 'SEO',
  'plugin.lighthouse.panel.score': '{score} ({rating})',
  'plugin.lighthouse.panel.rating.good': 'good',
  'plugin.lighthouse.panel.rating.poor': 'poor',
  'plugin.lighthouse.panel.noScore': 'no score',
  'plugin.lighthouse.panel.delta': '{delta} since the last run',
  'plugin.lighthouse.panel.running': 'Lighthouse is measuring {breakpoint}',
  'plugin.lighthouse.panel.queued': '{count} more waiting',
  'plugin.lighthouse.panel.skipped': 'Skipped: {breakpoint} ({reason})',
  'plugin.lighthouse.panel.reasons.engine': 'Lighthouse runs only in Chromium; this breakpoint uses {engine}',
  'plugin.lighthouse.panel.unavailable': 'Lighthouse is not available: {reason}',
  'plugin.lighthouse.errors.no_sidecar': 'The Node sidecar is not available ({reason}).',
  'plugin.lighthouse.panel.report': '{breakpoint}: {path}',
  'plugin.lighthouse.panel.findings': '{count, plural, one {# finding} other {# findings}}',
  'plugin.lighthouse.panel.idle': 'No Lighthouse run yet.',
  'plugin.lighthouse.panel.last': 'Run {run}: {url}',
  'plugin.lighthouse.panel.openPage': 'Open the page',
  'plugins.error.busy': 'The plugin is busy.',
  'plugins.panel.waiting': 'Waiting for the running check.',
};

const STATUS: LighthouseStatus = {
  sidecar: { available: true, running: { job: 'lh-2', label: 'desktop', url: 'http://x.test/' }, queued: 1 },
  latest: {
    runId: '20261112-110000',
    url: null,
    categories: ['performance', 'seo'],
    breakpoints: [
      { breakpoint: 'mobile', view: 1, width: 375, height: 812, formFactor: 'mobile', url: 'http://x.test/', scores: { performance: 48, seo: 92 }, report: '.rbrowser/out/lighthouse/r/mobile.html', json: '', findings: 7 },
      { breakpoint: 'desktop', view: 2, width: 1440, height: 900, formFactor: 'desktop', url: 'http://x.test/', scores: { performance: null, seo: 95 }, report: '.rbrowser/out/lighthouse/r/desktop.html', json: '', findings: 1 },
    ],
    skipped: [{ breakpoint: 'firefox', engine: 'firefox', reason: 'engine' }],
    previous: { runId: 'r0', breakpoints: [{ breakpoint: 'mobile', scores: { performance: 52, seo: 92 } }] },
  },
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

/** Answers every `status` call posted so far with `answer`. */
async function answerStatus(answer: LighthouseStatus | { error: string }): Promise<void> {
  for (const message of posted.filter((m) => m.type === 'call' && m.tool === 'status')) {
    window.dispatchEvent(
      new MessageEvent('message', {
        data: 'error' in answer ? { type: 'result', id: message.id, ok: false, error: answer.error } : { type: 'result', id: message.id, ok: true, value: answer },
        source: window,
      }),
    );
  }
  await settle();
}

const tile = (card: number, at: number): HTMLElement =>
  document.querySelectorAll('#rows [data-card]')[card]?.querySelectorAll('[data-tile]')[at] as HTMLElement;

beforeEach(() => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.lighthouse.', name: 'lighthouse panel' }).unmount;
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the Lighthouse panel', () => {
  it('asks for its texts, then for the status', async () => {
    expect(posted[0]).toEqual({ type: 'settings' });
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    expect(posted.some((m) => m.type === 'call' && m.tool === 'status')).toBe(true);
    await answerStatus(STATUS);
    expect($('#run')?.textContent).toBe('Run Lighthouse');
  });

  it('has Run as the main action and Cancel as an icon button with its name', async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    await answerStatus(STATUS);
    expect($('#title')?.textContent).toBe('Lighthouse');
    const cancel = $('#cancel') as HTMLButtonElement;
    expect(cancel.getAttribute('aria-label')).toBe('Cancel');
    expect(cancel.textContent).toBe('');
    expect($('#run')?.hasAttribute('data-primary')).toBe(true);
  });

  it('shows a card per breakpoint, its scores as tiles in words with the change since the last run', async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    await answerStatus(STATUS);
    const cards = [...document.querySelectorAll<HTMLElement>('#rows [data-card]')];
    expect(cards.map((card) => card.querySelector('h2')?.textContent)).toEqual(['mobile', 'desktop']);
    const mobile = cards[0] as HTMLElement;
    expect(mobile.getAttribute('aria-labelledby')).toBe(mobile.querySelector('h2')?.id);
    expect(mobile.querySelector('[data-findings]')?.textContent).toBe('7 findings');
    expect(cards[1]?.querySelector('[data-findings]')?.textContent).toBe('1 finding');
    const tiles = [...mobile.querySelectorAll<HTMLElement>('[data-tile]')];
    expect(tiles.map((one) => one.querySelector('[data-label]')?.textContent)).toEqual(['Performance', 'SEO']);
    expect(tiles.map((one) => one.dataset.tone)).toEqual(['error', 'ok']);
    expect(tiles.map((one) => one.querySelector('[data-value]')?.textContent)).toEqual(['48', '92']);
    expect(tiles.map((one) => one.querySelector('[data-rating]')?.textContent)).toEqual(['poor', 'good']);
    const delta = tiles[0]?.querySelector('[data-delta]') as HTMLElement;
    expect(delta.dataset.tone).toBe('error');
    expect(delta.querySelector('[data-icon]')?.getAttribute('data-icon')).toBe('arrow-down');
    expect(delta.textContent?.replace(/\s+/g, '')).toBe('-4-4sincethelastrun');
    expect(tiles[1]?.querySelector('[data-delta]')).toBeNull();
    expect(tiles[0]?.title).toBe('48 (poor), -4 since the last run');
    // RB-090-23 (WCAG 4.1.2): a focusable tile is a group named by what it shows.
    expect(tiles.map((one) => one.getAttribute('role'))).toEqual(['group', 'group']);
    expect(tiles[0]?.getAttribute('aria-label')).toBe('Performance: 48 (poor), -4 since the last run');
    expect(cards[1]?.querySelector('[data-tile]')?.getAttribute('aria-label')).toBe('Performance: no score');
    expect(cards[1]?.querySelector('[data-tile]')?.textContent).toBe('Performanceno score');
    const line = $('#status') as HTMLElement;
    expect(line.textContent).toBe('Lighthouse is measuring desktop - 1 more waiting');
    expect(line.dataset.tone).toBe('busy');
    expect($('#notes')?.textContent).toBe('Skipped: firefox (Lighthouse runs only in Chromium; this breakpoint uses firefox)');
    const report = $('#reports li') as HTMLElement;
    expect(report.textContent).toContain('mobile: .rbrowser/out/lighthouse/r/mobile.html');
    expect(report.querySelector('[data-icon="file-text"]')).not.toBeNull();
    expect(($('#cancel') as HTMLButtonElement).disabled).toBe(false);
  });

  it('lays the four tiles out four, two by two or one per row - never three and one', async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    await answerStatus(STATUS);
    const grid = document.querySelector<HTMLElement>('#rows [data-card] [data-tiles]');
    // jsdom has no container queries: the classes are the statement (owner, 2026-11-16).
    expect(grid?.parentElement?.classList.contains('@container')).toBe(true);
    expect([...(grid?.classList ?? [])].filter((name) => name.includes('grid-cols'))).toEqual([
      'grid-cols-1',
      '@min-[18rem]:grid-cols-2',
      '@min-[37rem]:grid-cols-4',
    ]);
  });

  it('moves the focus through the tiles with the arrow keys', async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    await answerStatus(STATUS);
    const first = tile(0, 0);
    expect(first.tabIndex).toBe(0);
    first.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    await settle();
    tile(0, 1).dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    await settle();
    const target = tile(1, 1);
    expect(target.tabIndex).toBe(0);
    expect(document.activeElement).toBe(target);
    expect(first.tabIndex).toBe(-1);
  });

  // Review of RB-080-62 (1): a status poll must not take the focus from the tile it stands on.
  it('keeps the focused tile across a status poll', async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    await answerStatus(STATUS);
    tile(0, 0).dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    await settle();
    expect(document.activeElement).toBe(tile(1, 0));
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    await answerStatus(STATUS);
    expect(document.activeElement).toBe(tile(1, 0));
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    await answerStatus({ ...STATUS, latest: { ...(STATUS.latest as LighthouseSummary), runId: 'r2' } });
    expect($('#last')?.textContent).toContain('r2');
    expect(document.activeElement).toBe(tile(1, 0));
    tile(1, 0).dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    await settle();
    expect(document.activeElement).toBe(tile(1, 1));
  });

  // Review of RB-080-62 (7): only a rating gives a tile its tone.
  it('gives a tile a tone only from a rating', async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    await answerStatus(STATUS);
    const tones = [...document.querySelectorAll<HTMLElement>('#rows [data-tile]')].map((one) => one.dataset.tone);
    expect(tones).toEqual(['error', 'ok', undefined, 'ok']);
  });

  it("takes the app's theme from its settings", async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'dark' } });
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'system' } });
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it('runs the command and cancels through the tool', async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    await answerStatus(STATUS);
    ($('#run') as HTMLButtonElement).click();
    expect(posted.some((m) => m.type === 'call' && m.command === 'run')).toBe(true);
    ($('#cancel') as HTMLButtonElement).click();
    expect(posted.some((m) => m.type === 'call' && m.tool === 'cancel')).toBe(true);
  });

  it('says a failed run in one sentence, never the code', async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    await answerStatus(STATUS);
    ($('#run') as HTMLButtonElement).click();
    const run = posted.find((m) => m.type === 'call' && m.command === 'run');
    await fromHost({ type: 'result', id: run?.id, ok: false, error: 'plugins.error.busy', params: { reason: 'no_answer' } });
    expect($('#status')?.textContent).toBe('The plugin is busy.');
    expect($('#status')?.dataset.tone).toBe('error');
  });

  it('opens the measured page through the host, only on a click', async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    await answerStatus(STATUS);
    expect(posted.some((m) => m.action === 'open_link')).toBe(false);
    ($('#open-page') as HTMLButtonElement).click();
    expect(posted.filter((m) => m.action === 'open_link')).toEqual([{ target: 'host', action: 'open_link', url: 'http://x.test/' }]);
  });

  it('says when the sidecar is not there', async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: {} });
    await answerStatus({
      sidecar: { available: false, error: 'plugin.lighthouse.errors.no_sidecar', reason: 'not started' },
      latest: null,
    });
    expect($('#status')?.textContent).toBe('Lighthouse is not available: The Node sidecar is not available (not started).');
    expect($('#status')?.dataset.tone).toBe('error');
    expect(document.querySelectorAll('#rows [data-card]').length).toBe(0);
    expect($('#empty')?.textContent).toBe('No Lighthouse run yet.');
  });

  it('ignores messages from anyone but the panels window', async () => {
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'settings', strings: STRINGS }, source: null }));
    await settle();
    expect(document.body.dataset.texts).toBe('waiting');
    expect(posted.some((m) => m.type === 'call')).toBe(false);
  });
});
