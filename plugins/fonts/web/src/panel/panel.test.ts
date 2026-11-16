// The Fonts panel in its frame, on Nuxt UI (RB-080-83): mounted as the panel frame mounts it; the
// panels window is played by spying on postMessage and dispatching `message` events from
// window.parent (the test window itself). The answer is the owner's page of 2026-11-14:
// "Rendered: unknown" in every row, a Firefox view without sizes.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import type { Answer } from './model';

type Posted = { type?: string; id?: number; tool?: string; command?: string; args?: Record<string, unknown> };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const STRINGS: Record<string, string> = {
  'plugin.fonts.panels.fonts': 'Fonts',
  'plugin.fonts.panel.refresh': 'Refresh',
  'plugin.fonts.panel.check': 'Check',
  'plugin.fonts.panel.export': 'Export',
  'plugin.fonts.panel.clear': 'Clear marks',
  'plugin.fonts.panel.provider.own': 'own server',
  'plugin.fonts.panel.provider.generic': 'generic',
  'plugin.fonts.panel.elementCount': '{count, plural, one {# element} other {# elements}}',
  'plugin.fonts.panel.findingCount': '{count, plural, one {# finding} other {# findings}}',
  'plugin.fonts.panel.faces': 'Faces',
  'plugin.fonts.panel.usage': 'Use',
  'plugin.fonts.panel.rendered': 'Rendered',
  'plugin.fonts.panel.state.loaded': 'loaded',
  'plugin.fonts.panel.state.error': 'failed',
  'plugin.fonts.panel.stateIn': '{state}: {views}',
  'plugin.fonts.panel.notScaled': 'not scaled',
  'plugin.fonts.panel.breakpoints': 'Font sizes per breakpoint',
  'plugin.fonts.panel.role': 'Role',
  'plugin.fonts.panel.flags': 'Notes',
  'plugin.fonts.panel.loading': 'Reading the views...',
  'plugin.fonts.panel.empty': 'No text and no web fonts in the views.',
  'plugin.fonts.panel.unsupported': 'No fonts from {views} (Chromium views only).',
  'plugin.fonts.panel.summary': '{views, plural, one {# view} other {# views}}, {families, plural, one {# font family} other {# font families}}.',
  'plugin.fonts.panel.marked': 'Marked {count, plural, one {# element} other {# elements}} in {views, plural, one {# view} other {# views}}.',
  'plugin.fonts.panel.kb': '{kb} KiB',
  'plugin.fonts.panel.preview.title': 'Preview',
  'plugin.fonts.panel.preview.too_large': 'No preview: the font file is larger than {kb} KiB.',
  'plugin.fonts.panel.preview.installed': 'Shown with the font installed on this computer, by its name.',
  'plugins.error.busy': 'The plugin is busy.',
  'plugins.panel.waiting': 'Waiting for the running check.',
};

const usage = (view: number, elements: number) => ({ view, elements, weights: [400, 700], styles: ['normal'], sizes: [14, 16] });

const ANSWER: Answer = {
  views: [
    { id: 1, label: 'md', width: 768, engine: 'chromium' },
    { id: 2, label: 'xl', width: 1280, engine: 'chromium' },
    { id: 3, label: 'mobile', width: 375, engine: 'firefox', error: 'unsupported' },
  ],
  families: [
    {
      family: 'Vision',
      provider: 'own',
      host: 'localhost',
      webFont: true,
      formats: ['woff2'],
      variable: false,
      fileKb: 45.2,
      faces: [
        {
          weight: '400', style: 'normal', display: 'swap', unicodeRange: '', src: 'http://localhost/v.woff2', format: 'woff2', kb: 45.2, preloaded: true,
          status: [{ view: 1, state: 'loaded' }, { view: 2, state: 'loaded' }],
        },
        {
          weight: '700', style: 'normal', display: 'swap', unicodeRange: '', src: 'http://localhost/vb.woff2', format: 'woff2', kb: 40, preloaded: false,
          status: [{ view: 1, state: 'loaded' }, { view: 2, state: 'error' }],
        },
      ],
      usage: [usage(1, 44), usage(2, 49)],
      rendered: [{ view: 1, state: 'unknown', as: [] }, { view: 2, state: 'unknown', as: [] }],
    },
    {
      family: 'ui-monospace', provider: 'generic', host: '', webFont: false, formats: [], variable: false, fileKb: null, faces: [],
      usage: [usage(1, 1)], rendered: [],
    },
  ],
  roles: [
    { role: 'a', views: [{ view: 1, width: 768, min: 16, max: 16 }, { view: 2, width: 1280, min: 16, max: 16 }], growth: 0, scaled: false, jump: false },
    { role: 'h1', views: [{ view: 1, width: 768, min: 48, max: 48 }, { view: 2, width: 1280, min: 72, max: 72 }], growth: 1.5, scaled: true, jump: false },
  ],
  findings: [],
};

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const texts = (selector: string): string[] => [...document.querySelectorAll(selector)].map((node) => (node.textContent ?? '').trim());
const settle = async (): Promise<void> => {
  for (let i = 0; i < 5; i += 1) await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
};

async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

/** Answers the panel's open `call`s of `tool` with `value` (or the failure `error`). */
async function answer(tool: string, value: unknown, error?: string): Promise<void> {
  for (const message of posted.filter((m) => m.type === 'call' && m.tool === tool)) {
    window.dispatchEvent(
      new MessageEvent('message', { data: error ? { type: 'result', id: message.id, ok: false, error } : { type: 'result', id: message.id, ok: true, value }, source: window }),
    );
  }
  posted = posted.filter((m) => !(m.type === 'call' && m.tool === tool));
  await settle();
}

beforeEach(async () => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.fonts.', name: 'fonts panel' }).unmount;
  // RB-090-44: the panels window's message limit, 1 MiB here; the panel asks for answers of it.
  await fromHost({
    type: 'settings',
    strings: STRINGS,
    settings: { 'fonts.previewText': 'Hamburgefonstiv', 'plugins.panelMessageMaxBytes': 1024 * 1024 },
  });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the Fonts panel', () => {
  it('has icon buttons named in its header, Check named as the main action', () => {
    expect($('#title')?.textContent).toBe('Fonts');
    for (const [id, label] of [['refresh', 'Refresh'], ['export', 'Export'], ['clear', 'Clear marks']]) {
      const button = $(`#${id}`) as HTMLButtonElement;
      expect(button.getAttribute('aria-label')).toBe(label);
      expect(button.textContent).toBe('');
      expect(button.querySelector('svg, [data-icon], span')).not.toBeNull();
    }
    expect($('#check')?.textContent).toBe('Check');
  });

  it('shows the loading state until the answer came, announced', async () => {
    expect($('#state')?.textContent).toContain('Reading the views...');
    expect($('#state')?.getAttribute('role')).toBe('status');
    expect($('#split')).toBeNull();
    expect(posted.find((m) => m.tool === 'list')?.args).toEqual({ budget: 1024 * 1024 - 5536 });
    await answer('list', ANSWER);
    expect($('#state')).toBeNull();
    expect($('#split')).not.toBeNull();
  });

  it('says what the views hold in one status line', async () => {
    await answer('list', ANSWER);
    expect($('#status')?.textContent).toBe('3 views, 2 font families. No fonts from mobile (Chromium views only).');
  });

  it('lists each family with its source and counts as badges, the use per view as a badge each', async () => {
    await answer('list', ANSWER);
    expect(texts('#families [data-name]')).toEqual(['Vision', 'ui-monospace']);
    expect(texts('#families [role="option"]:first-child [data-badges] > *')).toEqual(['own server', '45.2 KiB', '93 elements']);
    expect(texts('#families [role="option"]:first-child [data-per-view] > *')).toEqual(['md 44', 'xl 49']);
    expect(texts('#families [role="option"]:last-child [data-per-view] > *')).toEqual(['md 1']);
    expect($('#families')?.getAttribute('aria-activedescendant')).toBe('family-0');
    // RB-090-48: the tab row scrolls sideways only, never a vertical scroll bar beside the tabs.
    const row = [...(document.querySelector('#tabs [role="tablist"]')?.classList ?? [])];
    expect(row).toContain('overflow-y-hidden');
    expect(row.filter((name) => /^overflow-(y-)?(auto|scroll)$/.test(name))).toEqual([]);
  });

  it("reads a web font's file through the tool, never from the network, and shows an installed one by name", async () => {
    await answer('list', ANSWER);
    const preview = posted.find((m) => m.type === 'call' && m.tool === 'preview');
    expect(preview?.args).toEqual({ view: 1, src: 'http://localhost/v.woff2', offset: 0 });
    await answer('preview', { src: 'http://localhost/v.woff2', error: 'too_large', maxKb: 1024 });
    expect($('#families [role="option"]:first-child [data-preview-missing]')?.textContent?.trim()).toBe('No preview: the font file is larger than 1024 KiB.');
    const generic = $('#families [role="option"]:last-child [data-preview]') as HTMLElement;
    expect(generic.textContent?.trim()).toBe('Hamburgefonstiv');
    expect(generic.style.fontFamily).toBe('ui-monospace');
  });

  it("leaves out the Rendered column when no view could tell, and groups the faces' states", async () => {
    await answer('list', ANSWER);
    expect(texts('#detail [data-section="usage"] thead th')).not.toContain('Rendered');
    expect($('#detail')?.textContent).not.toContain('unknown');
    const rows = [...document.querySelectorAll('#detail [data-section="faces"] tbody tr')];
    const states = rows.map((row) => [...row.querySelectorAll('td:nth-child(4) > div > *')].map((badge) => (badge.textContent ?? '').trim()));
    expect(states[0]).toEqual(['loaded']);
    expect(states[1]).toEqual(expect.arrayContaining(['loaded: md', 'failed: xl']));
  });

  it('keeps the open sections from family to family', async () => {
    await answer('list', ANSWER);
    const openOnes = (): string[] => [...document.querySelectorAll('#detail button[data-slot="trigger"][data-state="open"]')].map((button) => (button.textContent ?? '').trim());
    expect(openOnes()).toEqual(expect.arrayContaining(['Preview', 'Faces']));
    const faces = [...document.querySelectorAll<HTMLButtonElement>('#detail button[data-slot="trigger"]')].find((button) => button.textContent?.includes('Faces'));
    faces?.click();
    await settle();
    expect(openOnes()).not.toContain('Faces');
    $('#families')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    await settle();
    expect($('#families')?.getAttribute('aria-activedescendant')).toBe('family-1');
    expect($('#detail h2')?.textContent).toBe('ui-monospace');
    expect($('#detail [data-section="preview"]')?.textContent).toContain('Shown with the font installed on this computer, by its name.');
    expect(openOnes()).toEqual(['Preview']);
  });

  it('puts "not scaled" in a column of its own, without a column of dashes for the Firefox view', async () => {
    await answer('list', ANSWER);
    expect(texts('#roles thead th')).toEqual(['Role', 'md', 'xl', 'Notes']);
    const first = [...(document.querySelector('#roles tbody tr')?.children ?? [])].map((cell) => (cell.textContent ?? '').trim());
    expect(first).toEqual(['a', '16', '16', 'not scaled']);
    expect($('#roles tbody tr [data-tone]')?.getAttribute('data-tone')).toBe('warn');
  });

  it('marks the family on Enter and says so in the status line', async () => {
    await answer('list', ANSWER);
    $('#families')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    const mark = posted.find((m) => m.type === 'call' && m.tool === 'mark');
    expect(mark?.args).toEqual({ family: 'Vision' });
    await answer('mark', { marked: [{ count: 44 }, { count: 49 }] });
    expect($('#status')?.textContent).toBe('Marked 93 elements in 2 views.');
    expect($('#status')?.dataset.tone).toBe('info');
  });

  it('marks the family a click selects', async () => {
    await answer('list', ANSWER);
    ($('#family-1') as HTMLElement).click();
    await settle();
    expect(posted.find((m) => m.type === 'call' && m.tool === 'mark')?.args).toEqual({ family: 'ui-monospace' });
  });

  it("takes the app's theme", async () => {
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'dark' } });
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    await fromHost({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'system' } });
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  // docs/design.md 5.6: loading only on the first fetch; a failure is neither loading nor empty.
  it('keeps the families while it reads again, and a failure is no loading or empty state', async () => {
    await answer('list', ANSWER);
    ($('#refresh') as HTMLButtonElement).click();
    await settle();
    expect($('#state')).toBeNull();
    expect(texts('#families [data-name]')).toEqual(['Vision', 'ui-monospace']);
    await answer('list', null, 'plugins.error.busy');
    expect($('#status')?.dataset.tone).toBe('error');
    expect($('#status')?.textContent).toBe('The plugin is busy.');
    expect(texts('#families [data-name]')).toEqual(['Vision', 'ui-monospace']);
  });

  it('says a failed first read in the status line in words, without the loading state', async () => {
    await answer('list', null, 'plugins.error.busy');
    expect($('#status')?.textContent).toBe('The plugin is busy.');
    expect($('#state')?.textContent).toBe('');
  });

  it("shows the host's waiting while a call waits for a running check", async () => {
    await answer('list', ANSWER);
    ($('#check') as HTMLButtonElement).click();
    await fromHost({ type: 'event', message: { host: 'waiting', plugin: 'fonts' } });
    expect($('#status')?.textContent).toBe('Waiting for the running check.');
    expect($('#status')?.dataset.tone).toBe('busy');
  });

  it('shows the empty state for a page without fonts', async () => {
    await answer('list', { ...ANSWER, families: [], roles: [] });
    expect($('#state')?.textContent).toContain('No text and no web fonts in the views.');
    expect($('#split')).toBeNull();
  });
});
