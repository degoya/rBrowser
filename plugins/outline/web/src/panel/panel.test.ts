// The Outline panel in its frame (RB-080-85): mounted as the panel frame mounts it; the panels
// window is played by spying on postMessage and dispatching `message` events from window.parent
// (the test window itself). The owner's word: sections as tabs, Refresh and Clear marks set apart,
// one status line.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import en from '../../../locales/en.json';
import App from './App.vue';
import type { Answer } from './model';

type Posted = { type?: string; id?: number; tool?: string; args?: Record<string, unknown> };

function flatten(value: unknown, prefix = ''): Record<string, string> {
  if (typeof value === 'string') return { [prefix]: value };
  return Object.entries(value as Record<string, unknown>).reduce<Record<string, string>>(
    (all, [key, child]) => ({ ...all, ...flatten(child, prefix ? `${prefix}.${key}` : key) }),
    {},
  );
}
const STRINGS = { ...flatten(en), 'plugins.panel.call_failed': 'The plugin could not do that.' };

const cell = (path: string, visible = true) => ({ index: 0, visible, hidden: !visible, selector: `#${path}`, path });

const HEADINGS: Answer = {
  views: [
    { id: 1, label: 'Desktop' },
    { id: 2, label: 'Phone' },
    { id: 3, label: 'mobile', error: 'firefox' },
  ],
  findings: [{ rule: 'heading-skip', severity: 'warning', message: 'plugin.outline.findings.heading-skip', params: { from: 'h1', to: 'h3' }, view: 1, path: 'h3' }],
  sections: {
    headings: {
      rows: [
        { item: { level: 1, text: 'Title' }, views: { '1': cell('h1'), '2': cell('h1') } },
        { item: { level: 3, text: 'Deep' }, views: { '1': cell('h3') }, only: [1] },
      ],
    },
  },
};

const LINKS: Answer = {
  views: [{ id: 1, label: 'Desktop' }],
  findings: [],
  sections: { links: { rows: [{ item: { text: 'Docs', href: 'https://a.test/docs#x', internal: false }, views: { '1': cell('a') } }] } },
};

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

/** Answers the calls of `tool` not answered yet. */
const answered = new Set<number>();
async function answer(tool: string, value: unknown, ok = true): Promise<void> {
  for (const message of posted.filter((m) => m.type === 'call' && m.tool === tool && !answered.has(m.id ?? 0))) {
    answered.add(message.id ?? 0);
    await fromHost(ok ? { type: 'result', id: message.id, ok: true, value } : { type: 'result', id: message.id, ok: false, error: value });
  }
}

const rows = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('#tree [role="treeitem"]')];

beforeEach(async () => {
  posted = [];
  answered.clear();
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.outline.', name: 'outline panel' }).unmount;
  // RB-090-44: the panels window's message limit, 1 MiB here; the panel asks for answers of it.
  await fromHost({ type: 'settings', strings: STRINGS, settings: { 'plugins.panelMessageMaxBytes': 1024 * 1024 } });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the Outline panel', () => {
  it('keeps its tabs in one row without a vertical scroll bar', () => {
    // RB-090-48: the tab row scrolls sideways only, never a vertical scroll bar beside the tabs.
    const row = [...(document.querySelector('#tabs [role="tablist"]')?.classList ?? [])];
    expect(row).toContain('overflow-y-hidden');
    expect(row.filter((name) => /^overflow-(y-)?(auto|scroll)$/.test(name))).toEqual([]);
  });

  it('shows the sections as tabs, not as a row of buttons', () => {
    const tabs = [...document.querySelectorAll<HTMLElement>('#tabs [role="tab"]')];
    expect(tabs.map((tab) => tab.textContent?.trim())).toEqual(['Headings', 'Landmarks', 'Links', 'Images', 'Forms', 'Focus', 'Breakpoints', 'Reading order', 'A11y tree', 'Custom']);
    expect(tabs[0]?.getAttribute('aria-selected')).toBe('true');
  });

  it('sets Refresh and Clear marks apart as header actions; Clear only while something is marked', async () => {
    const refresh = $('#refresh') as HTMLButtonElement;
    expect(refresh.getAttribute('aria-label')).toBe('Refresh');
    expect(refresh.textContent).toBe('');
    const clear = $('#clear') as HTMLButtonElement;
    expect(clear.getAttribute('aria-label')).toBe('Clear marks');
    expect(clear.className).not.toBe(refresh.className);
    expect(clear.disabled).toBe(true);
    await answer('get', HEADINGS);
    rows()[0]?.click();
    await settle();
    expect(posted.find((m) => m.tool === 'mark')?.args).toEqual({ targets: [{ view: 1, selector: '#h1' }, { view: 2, selector: '#h1' }] });
    await answer('mark', { marked: [{ found: true }, { found: true }] });
    expect(clear.disabled).toBe(false);
    clear.click();
    await answer('mark', {});
    expect(posted.filter((m) => m.tool === 'mark').at(-1)?.args).toEqual({ clear: true });
    expect(clear.disabled).toBe(true);
  });

  it('says loading, then shows the tree with a dot per view, the badge and the finding', async () => {
    expect($('#status')?.textContent).toBe('Reading the views…');
    expect(posted.find((m) => m.tool === 'get')?.args).toEqual({ sections: ['headings'], budget: 1024 * 1024 - 5536 });
    await answer('get', HEADINGS);
    expect(rows().map((row) => row.querySelector('[data-label]')?.textContent)).toEqual(['H1 Title', 'H3 Deep']);
    expect(rows()[0]?.getAttribute('aria-level')).toBe('1');
    const dots = [...(rows()[0]?.querySelectorAll<HTMLElement>('[data-dots] [role="img"]') ?? [])];
    expect(dots.map((dot) => dot.getAttribute('aria-label'))).toEqual(['Desktop: visible', 'Phone: visible', 'mobile: missing']);
    expect(rows()[1]?.querySelector('[data-badge]')?.textContent).toBe('only Desktop');
    expect(rows()[1]?.querySelector('[data-note]')?.textContent).toContain('h1');
  });

  it('says everything in one status line with its most urgent tone', async () => {
    await answer('get', HEADINGS);
    const line = $('#status') as HTMLElement;
    expect(line.textContent).toBe('No outline from mobile (Chromium views only). 1 finding in this section.');
    expect(line.dataset.tone).toBe('warn');
    rows()[0]?.click();
    await settle();
    await answer('mark', { marked: [{ found: true }, { found: false }] });
    expect(line.textContent).toBe('Marked in 1 of 2 views. No outline from mobile (Chromium views only). 1 finding in this section.');
  });

  it('marks with the keyboard (Enter) and keeps no selection of its own', async () => {
    await answer('get', HEADINGS);
    const first = rows()[0] as HTMLElement;
    first.focus();
    first.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await settle();
    expect(posted.some((m) => m.tool === 'mark')).toBe(true);
    expect(first.getAttribute('aria-selected')).not.toBe('true');
  });

  it('loads a section on its tab and shows an empty one as such', async () => {
    await answer('get', HEADINGS);
    (document.querySelectorAll<HTMLElement>('#tabs [role="tab"]')[1] as HTMLElement).dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
    await settle();
    const get = posted.filter((m) => m.tool === 'get').at(-1);
    expect(get?.args).toEqual({ sections: ['landmarks'], budget: 1024 * 1024 - 5536 });
    await answer('get', { views: [{ id: 1, label: 'Desktop' }], findings: [], sections: { landmarks: { rows: [] } } });
    expect($('#empty')?.textContent).toBe('Nothing in this section.');
  });

  it('checks the links on the Links tab and shows the state at the link', async () => {
    await answer('get', HEADINGS);
    (document.querySelectorAll<HTMLElement>('#tabs [role="tab"]')[2] as HTMLElement).dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
    await settle();
    expect($('#check-links')).toBeNull();
    await answer('get', LINKS);
    ($('#check-links') as HTMLButtonElement).click();
    await settle();
    expect(posted.find((m) => m.tool === 'check_links')?.args).toEqual({ urls: ['https://a.test/docs'] });
    await answer('check_links', { results: [{ url: 'https://a.test/docs', state: 'broken', status: 404 }] });
    expect(rows()[0]?.querySelector('[data-link]')?.textContent).toBe('broken (404)');
    expect($('#status')?.textContent).toBe('Checked 1 of 1 link.');
  });

  it('says a failed call in one sentence, never the code', async () => {
    await answer('get', 'plugins.error.busy', false);
    expect($('#status')?.textContent).toBe('The plugin could not do that.');
    expect($('#status')?.dataset.tone).toBe('error');
  });

  it("takes the app's theme from its settings", async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'dark' } });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });
});

describe('the findings in the Outline panel (RB-091-03)', () => {
  const missing = (view: number) => ({
    rule: 'h1-missing',
    severity: 'warning' as const,
    message: 'plugin.outline.findings.h1-missing',
    params: {},
    view,
    path: 'html',
    selector: 'html',
  });
  const PAGE: Answer = { ...HEADINGS, findings: [...HEADINGS.findings, missing(1), missing(2)] };

  it('colours a finding by its severity and lists the page\'s findings above the tree; a click marks them', async () => {
    await answer('get', PAGE);
    expect(rows()[1]?.querySelector('[data-note]')?.getAttribute('data-severity')).toBe('warning');
    const notes = [...document.querySelectorAll<HTMLElement>('#page-findings [data-page-note]')];
    expect(notes.map((note) => note.textContent?.trim())).toEqual([en.plugin.outline.findings['h1-missing']]);
    notes[0]?.click();
    await settle();
    expect(posted.find((m) => m.tool === 'mark')?.args).toEqual({ targets: [{ view: 1, selector: 'html' }, { view: 2, selector: 'html' }] });
    expect($('#status')?.textContent).toContain('2 findings in this section.');
  });

  it('keeps only the entries with a finding while "Only problems" is on', async () => {
    await answer('get', HEADINGS);
    expect(rows()).toHaveLength(2);
    ($('#problems') as HTMLElement).click();
    await settle();
    expect(rows().map((row) => row.querySelector('[data-label]')?.textContent)).toEqual(['H3 Deep']);
  });

  it('counts each tab\'s findings from one findings-only call before the tab is read', async () => {
    const summary = posted.find((m) => m.tool === 'get' && m.args?.findingsOnly === true);
    expect(summary?.args).toEqual({ findingsOnly: true, budget: 1024 * 1024 - 5536 });
    answered.add(summary?.id ?? 0);
    await fromHost({
      type: 'result',
      id: summary?.id,
      ok: true,
      value: {
        views: HEADINGS.views,
        findings: [
          { rule: 'landmark-main-missing', severity: 'warning', message: 'plugin.outline.findings.landmark-main-missing', params: {}, view: 1, path: 'html' },
          { rule: 'landmark-main-missing', severity: 'warning', message: 'plugin.outline.findings.landmark-main-missing', params: {}, view: 2, path: 'html' },
          { rule: 'link-empty', severity: 'error', message: 'plugin.outline.findings.link-empty', params: {}, view: 1, path: 'a' },
        ],
      },
    });
    const tabs = [...document.querySelectorAll<HTMLElement>('#tabs [role="tab"]')].map((tab) => tab.textContent?.replace(/\s+/g, ''));
    expect(tabs.slice(0, 4)).toEqual(['Headings', 'Landmarks1', 'Links1', 'Images']);
  });
});

describe('the tabs over the breakpoints (RB-094-02)', () => {
  const tabNamed = (name: string): HTMLElement =>
    [...document.querySelectorAll<HTMLElement>('#tabs [role="tab"]')].find((tab) => tab.textContent?.trim() === name) as HTMLElement;
  const open = async (name: string): Promise<void> => {
    tabNamed(name).dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
    await settle();
  };

  it('reads the Breakpoints tab from breakpoint_diff and shows each element with the breakpoints missing it', async () => {
    await answer('get', HEADINGS);
    await open('Breakpoints');
    expect(posted.filter((m) => m.type === 'call').at(-1)).toMatchObject({ tool: 'breakpoint_diff', args: { budget: 1024 * 1024 - 5536 } });
    await answer('breakpoint_diff', {
      views: [
        { id: 1, label: 'Desktop' },
        { id: 2, label: 'Phone' },
      ],
      findings: [],
      rows: [{ kind: 'interactive', label: 'link "Pricing"', views: { '1': cell('pricing') }, shown: [1], missing: [2] }],
    });
    expect(rows().map((row) => row.querySelector('[data-label]')?.textContent)).toEqual(['Interactive elements', 'link "Pricing"']);
    expect(rows()[1]?.querySelector('[data-badge]')?.textContent).toBe('missing in Phone');
    expect($('#problems')).toBeNull();
  });

  it('says so when every view shows the same elements', async () => {
    await answer('get', HEADINGS);
    await open('Breakpoints');
    await answer('breakpoint_diff', { views: [{ id: 1, label: 'Desktop' }], findings: [], rows: [] });
    expect($('#empty')?.textContent).toBe('Every view shows the same elements.');
  });

  it('shows the tab a command posts and reads it from reading_order', async () => {
    await answer('get', HEADINGS);
    await fromHost({ type: 'event', message: { type: 'show', tab: 'order' } });
    expect(tabNamed('Reading order').getAttribute('aria-selected')).toBe('true');
    expect(posted.filter((m) => m.type === 'call').at(-1)?.tool).toBe('reading_order');
    await answer('reading_order', {
      views: [{ id: 1, label: 'Desktop' }],
      findings: [
        {
          rule: 'reading-order',
          severity: 'warning',
          message: 'plugin.outline.findings.reading-order',
          params: { from: 'button "A"', to: 'button "B"' },
          view: 1,
          path: 'b',
          selector: '#b',
          data: { other: { selector: '#a', path: 'a' } },
        },
      ],
    });
    expect(rows().map((row) => row.querySelector('[data-label]')?.textContent)).toEqual([
      'Desktop',
      'The button "B" comes first on the screen but after the button "A" in the source, which screen readers and the keyboard follow (WCAG 1.3.2).',
    ]);
    rows()[1]?.click();
    await settle();
    expect(posted.find((m) => m.tool === 'mark')?.args).toEqual({ targets: [{ view: 1, selector: '#b' }, { view: 1, selector: '#a' }] });
  });
});
