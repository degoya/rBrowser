// The panel in a window of its own: the bridge messages it sends and how it renders the
// component's answers. The panels window is played by spying on postMessage and dispatching
// `message` events from window.parent (the test window itself).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import { APPLY_DELAY_MS, type DraftState } from './panel/model';

type Posted = { type: string; id?: number; tool?: string; args?: Record<string, unknown>; dirty?: boolean };

const MD_COPY = '@media (min-width: 768px) and (max-width: 1279px) {\n  body { outline: 3px solid red }\n}';

const STRINGS: Record<string, string> = {
  'plugin.live-css.panels.editor': 'Live CSS',
  'plugin.live-css.panel.block': 'Block {n}',
  'plugin.live-css.panel.breakpoint': '{label} ({width} px)',
  'plugin.live-css.panel.missing': '{id} (not in the grid)',
  'plugin.live-css.panel.allViews': 'Every view',
  'plugin.live-css.panel.nowhere': 'No view',
  'plugin.live-css.panel.copied': 'Copied block {n}.',
  'plugin.live-css.panel.notApplied': 'Not applied: {reason}',
  'plugin.live-css.panel.untrusted': 'Trust the workspace to use Live CSS.',
  'plugin.live-css.panel.confirmRemove': 'Remove block {n}?',
  'plugin.live-css.panel.empty': 'No CSS yet.',
  'plugin.live-css.panel.file': 'Kept in {file}.',
  'plugin.live-css.errors.braces': 'The braces of block {block} do not balance.',
  'plugins.error.busy': 'The plugin is busy.',
};

const STATE: DraftState = {
  available: 'trusted',
  enabled: true,
  blocks: [
    { css: 'body { margin: 0 }', breakpoints: [], scope: 'all', media: null, copy: 'body { margin: 0 }' },
    {
      css: 'body { outline: 3px solid red }',
      breakpoints: ['md', 'xl'],
      scope: 'query',
      media: '(min-width: 768px) and (max-width: 1279px)',
      copy: MD_COPY,
    },
  ],
  breakpoints: [
    { id: 'mobile', label: 'Mobile', width: 375 },
    { id: 'md', label: 'MD', width: 768 },
    { id: 'lg', label: 'LG', width: 1280 },
  ],
  file: '.rbrowser/out/live-css/draft.css',
};

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const settle = async (): Promise<void> => {
  for (let i = 0; i < 3; i++) {
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
  }
};
/** Waits past the delay after the last keystroke (a lower bound only). */
const pause = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, APPLY_DELAY_MS + 50));

async function host(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

function calls(tool: string): Posted[] {
  return posted.filter((m) => m.type === 'call' && m.tool === tool);
}

async function reply(call: Posted | undefined, value: unknown): Promise<void> {
  expect(call, 'a call').toBeDefined();
  await host({ type: 'result', id: call?.id, ok: true, value });
}

async function refuse(call: Posted | undefined, error: string, params?: Record<string, unknown>): Promise<void> {
  expect(call, 'a call').toBeDefined();
  await host({ type: 'result', id: call?.id, ok: false, error, params });
}

const $ = (selector: string) => document.querySelector(selector) as HTMLElement;
const cards = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('[data-block]')];
const editor = (n: number): HTMLTextAreaElement => cards()[n - 1]?.querySelector('textarea') as HTMLTextAreaElement;
const boxes = (n: number): HTMLElement[] => [...(cards()[n - 1]?.querySelectorAll<HTMLElement>('[role="checkbox"]') ?? [])];
const status = (): string => $('#status').textContent?.trim() ?? '';

async function type(field: HTMLTextAreaElement, value: string): Promise<void> {
  field.value = value;
  field.dispatchEvent(new Event('input', { bubbles: true }));
  await settle();
}

async function open(state: DraftState = STATE): Promise<void> {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  unmount = mountPanel(App, { prefix: 'plugin.live-css.', name: 'Live CSS panel' }).unmount;
  await host({ type: 'settings', strings: STRINGS, settings: {} });
  await reply(calls('set').at(-1), state);
}

beforeEach(() => {
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
  document.body.innerHTML = '';
});

describe('the Live CSS panel', () => {
  it('asks for its texts, then the draft, and shows each block with where it applies', async () => {
    await open();
    expect(posted[0]).toEqual({ type: 'settings' });
    expect(calls('set')[0]?.args).toEqual({});
    expect(document.title).toBe('Live CSS');
    expect(cards().map((card) => card.querySelector('h2')?.textContent)).toEqual(['Block 1', 'Block 2']);
    expect(editor(2).value).toBe('body { outline: 3px solid red }');
    expect(cards().map((card) => card.querySelector('[data-where]')?.textContent?.trim())).toEqual([
      'Every view',
      '@media (min-width: 768px) and (max-width: 1279px)',
    ]);
    expect($('#file').textContent).toBe('Kept in .rbrowser/out/live-css/draft.css.');
    expect(status()).toBe('');
  });

  it('offers the grid breakpoints by name, the chosen ones checked, a gone one kept and marked', async () => {
    await open();
    const labels = [...(cards()[1]?.querySelectorAll('[data-scope] label') ?? [])].map((label) => label.textContent?.trim());
    expect(labels).toEqual(['Mobile (375 px)', 'MD (768 px)', 'LG (1280 px)', 'xl (not in the grid)']);
    expect(boxes(2).map((box) => box.getAttribute('aria-checked'))).toEqual(['false', 'true', 'false', 'true']);
  });

  it('hands the draft over a moment after the last keystroke, unsaved until stored', async () => {
    await open();
    await type(editor(1), 'body { margin: 4px }');
    expect(calls('set')).toHaveLength(1);
    expect(posted.filter((m) => m.type === 'dirty').at(-1)).toEqual({ type: 'dirty', dirty: true });
    await pause();
    const sent = calls('set').at(-1);
    expect(sent?.args).toEqual({
      blocks: [
        { css: 'body { margin: 4px }', breakpoints: [] },
        { css: 'body { outline: 3px solid red }', breakpoints: ['md', 'xl'] },
      ],
      enabled: true,
    });
    await reply(sent, STATE);
    expect(posted.filter((m) => m.type === 'dirty').at(-1)).toEqual({ type: 'dirty', dirty: false });
  });

  it('sends a breakpoint chosen in the group', async () => {
    await open();
    boxes(1)[2]?.click();
    await settle();
    await pause();
    expect(calls('set').at(-1)?.args?.blocks).toEqual([
      { css: 'body { margin: 0 }', breakpoints: ['lg'] },
      { css: 'body { outline: 3px solid red }', breakpoints: ['md', 'xl'] },
    ]);
  });

  it('keeps a refused draft in the panel and says why', async () => {
    await open();
    await type(editor(2), '} body {');
    await pause();
    await refuse(calls('set').at(-1), 'plugin.live-css.errors.braces', { block: 2 });
    expect(status()).toBe('Not applied: The braces of block 2 do not balance.');
    expect(editor(2).value).toBe('} body {');
    expect(posted.filter((m) => m.type === 'dirty').at(-1)).toEqual({ type: 'dirty', dirty: true });
  });

  it('switches the draft off at once, the blocks kept', async () => {
    await open();
    $('#enabled').click();
    await settle();
    expect(calls('set').at(-1)?.args).toEqual({
      blocks: [
        { css: 'body { margin: 0 }', breakpoints: [] },
        { css: 'body { outline: 3px solid red }', breakpoints: ['md', 'xl'] },
      ],
      enabled: false,
    });
  });

  it('adds a block, and removes one only after asking', async () => {
    await open();
    ($('#add') as HTMLButtonElement).click();
    await settle();
    expect(cards()).toHaveLength(3);
    (cards()[0]?.querySelector('[data-remove]') as HTMLButtonElement).click();
    await settle();
    expect(cards()[0]?.querySelector('[data-confirm]')?.textContent).toContain('Remove block 1?');
    expect(document.activeElement?.hasAttribute('data-keep')).toBe(true);
    (cards()[0]?.querySelector('[data-keep]') as HTMLButtonElement).click();
    await settle();
    expect(cards()).toHaveLength(3);
    (cards()[0]?.querySelector('[data-remove]') as HTMLButtonElement).click();
    await settle();
    (cards()[0]?.querySelector('[data-confirm-remove]') as HTMLButtonElement).click();
    await settle();
    expect(editor(1).value).toBe('body { outline: 3px solid red }');
    await pause();
    expect(calls('set').at(-1)?.args?.blocks).toEqual([
      { css: 'body { outline: 3px solid red }', breakpoints: ['md', 'xl'] },
      { css: '', breakpoints: [] },
    ]);
  });

  it('copies a block as media query with the bounds the component answered', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    await open();
    (cards()[1]?.querySelector('[data-copy]') as HTMLButtonElement).click();
    await settle();
    expect(writeText).toHaveBeenCalledWith(MD_COPY);
    expect(status()).toBe('Copied block 2.');
  });

  it('copies what was typed: the edit is handed over first', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    await open();
    await type(editor(1), 'a { color: red }');
    (cards()[0]?.querySelector('[data-copy]') as HTMLButtonElement).click();
    await settle();
    const sent = calls('set').at(-1);
    expect(sent?.args?.blocks).toEqual([
      { css: 'a { color: red }', breakpoints: [] },
      { css: 'body { outline: 3px solid red }', breakpoints: ['md', 'xl'] },
    ]);
    expect(writeText).not.toHaveBeenCalled();
    const blocks = [{ ...STATE.blocks[0], css: 'a { color: red }', copy: 'a { color: red }' }, STATE.blocks[1]];
    await reply(sent, { ...STATE, blocks });
    expect(writeText).toHaveBeenCalledWith('a { color: red }');
  });

  it('reloads the draft on a change from elsewhere unless it holds edits', async () => {
    await open();
    await host({ type: 'event', message: { changed: true } });
    expect(calls('set')).toHaveLength(2);
    await reply(calls('set').at(-1), { ...STATE, enabled: false, blocks: [] });
    expect(cards()).toHaveLength(0);
    expect(($('#enabled') as HTMLElement).getAttribute('aria-checked')).toBe('false');
    expect($('#empty').textContent).toContain('No CSS yet.');
    ($('#add-first') as HTMLButtonElement).click();
    await settle();
    await type(editor(1), 'a{}');
    await host({ type: 'event', message: { changed: true } });
    expect(calls('set')).toHaveLength(2);
  });

  it('outside a trusted workspace says why and edits nothing', async () => {
    await open({ ...STATE, available: 'untrusted', blocks: [] });
    expect(status()).toBe('Trust the workspace to use Live CSS.');
    expect(($('#add') as HTMLButtonElement).disabled).toBe(true);
    expect($('#enabled').hasAttribute('disabled') || $('#enabled').getAttribute('aria-disabled') === 'true').toBe(true);
    expect(document.querySelector('#empty')).toBeNull();
    expect(document.querySelector('#file')).toBeNull();
  });

  it('says when the draft could not be read and offers Retry', async () => {
    posted = [];
    document.body.innerHTML = '<div id="app"></div>';
    unmount = mountPanel(App, { prefix: 'plugin.live-css.', name: 'Live CSS panel' }).unmount;
    await host({ type: 'settings', strings: STRINGS, settings: {} });
    await refuse(calls('set').at(-1), 'plugins.error.busy');
    expect(status()).toBe('The plugin is busy.');
    expect($('#failed').getAttribute('role')).toBe('alert');
    $('#retry').click();
    await settle();
    await reply(calls('set').at(-1), STATE);
    expect(cards()).toHaveLength(2);
    expect(status()).toBe('');
  });
});
