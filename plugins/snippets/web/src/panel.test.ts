// The panel in a window of its own: the bridge messages it sends and how it renders the
// plugin's answers. The panels window is played by spying on postMessage and dispatching
// `message` events from window.parent (the test window itself). On Nuxt UI since RB-080-84.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';
import type { ListAnswer } from './panel/model';

type Posted = { type: string; id?: number; tool?: string; args?: Record<string, unknown> };

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const STRINGS: Record<string, string> = {
  'plugin.snippets.panels.snippets': 'Snippets',
  'plugin.snippets.panel.projectUntrusted': 'Trust the workspace to load the project snippets.',
  'plugin.snippets.panel.viewList': 'Views: {views}',
  'plugin.snippets.panel.source.user': 'Yours',
  'plugin.snippets.panel.source.project': 'Project',
  'plugin.snippets.panel.source.local': 'Saved',
  'plugin.snippets.panel.invalid': 'Cannot run: {reason}',
  'plugin.snippets.panel.running': 'Running in {count, plural, one {# view} other {# views}}...',
  'plugin.snippets.panel.saved': 'Saved "{name}".',
  'plugin.snippets.panel.stack': 'Stack trace',
  'plugin.snippets.panel.delete': 'Delete',
  'plugin.snippets.panel.confirmDelete': 'Delete "{name}"?',
  'plugin.snippets.panel.keep': 'Keep',
  'plugin.snippets.errors.header': 'Bad header: @{tag}.',
  'plugin.snippets.errors.save_untrusted': 'Saving needs trust.',
  'plugin.snippets.panel.setting_link': 'Open the setting',
  'plugin.snippets.errors.no_active_view': 'No view is active.',
  'plugins.error.busy': 'The plugin is busy.',
};

const LIST: ListAnswer = {
  snippets: [
    { source: 'user', name: 'Title', description: 'The title', world: 'isolated', target: 'all', timeoutMs: 5000, code: 'return document.title' },
    { source: 'project', name: 'Bad', description: '', world: 'isolated', target: 'all', timeoutMs: 5000, code: '// @world x',
      error: { code: 'plugin.snippets.errors.header', params: { tag: 'world' } } },
  ],
  project: 'untrusted',
  local: 'untrusted',
  views: [
    { id: 1, label: 'Mobile', breakpoint: 'mobile', engine: 'chromium' },
    { id: 2, label: 'Desktop', breakpoint: 'desktop', engine: 'chromium' },
  ],
  active: 2,
};

const settle = async (): Promise<void> => {
  for (let i = 0; i < 3; i++) {
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
  }
};

async function host(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}

function calls(tool: string): Posted[] {
  return posted.filter((m) => m.type === 'call' && m.tool === tool);
}

async function reply(call: Posted | undefined, value: unknown, ok = true): Promise<void> {
  expect(call, 'a call').toBeDefined();
  await host(ok ? { type: 'result', id: call?.id, ok, value } : { type: 'result', id: call?.id, ok, error: value });
}

const $ = (id: string) => document.getElementById(id) as HTMLElement;
const rows = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('#list [role="option"]')];
const code = (): HTMLTextAreaElement => $('code') as HTMLTextAreaElement;

/** Types `value` into a field, as a person does. */
async function type(field: HTMLInputElement | HTMLTextAreaElement, value: string): Promise<void> {
  field.value = value;
  field.dispatchEvent(new Event('input', { bubbles: true }));
  await settle();
}

/** Clicks a snippet and lets the panel draw. */
async function pick(index: number): Promise<void> {
  rows()[index]?.click();
  await settle();
}

beforeEach(async () => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.snippets.', name: 'snippets panel' }).unmount;
  await host({ type: 'settings', strings: STRINGS, settings: {} });
  await reply(calls('list').at(-1), LIST);
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
  document.body.innerHTML = '';
});

describe('the snippets panel', () => {
  it('asks for its texts, then the list with the code', () => {
    expect(posted[0]).toEqual({ type: 'settings' });
    expect(calls('list')[0]?.args).toEqual({ withCode: true });
    expect(document.title).toBe('Snippets');
    expect($('title').textContent).toBe('Snippets');
  });

  it('lists the snippets as options, says why one cannot run and why the project is missing', () => {
    expect($('list').getAttribute('role')).toBe('listbox');
    expect(rows().map((row) => row.querySelector('[data-name]')?.textContent)).toEqual(['Title', 'Bad']);
    expect(rows().map((row) => row.querySelector('[data-source]')?.textContent)).toEqual(['Yours', 'Project']);
    expect(rows()[0]?.getAttribute('role')).toBe('option');
    expect(rows()[0]?.getAttribute('tabindex')).toBe('0');
    expect(rows()[1]?.querySelector('[data-invalid]')?.textContent).toBe('Cannot run: Bad header: @world.');
    expect($('status').textContent).toBe('Trust the workspace to load the project snippets.');
    expect($('status').dataset.tone).toBe('info');
    expect(($('save') as HTMLButtonElement).disabled).toBe(true);
    expect(($('delete') as HTMLButtonElement).disabled).toBe(true);
  });

  it('runs a stored snippet view by view and joins the views answers', async () => {
    await pick(0);
    expect(code().value).toBe('return document.title');
    expect(rows()[0]?.getAttribute('aria-selected')).toBe('true');
    $('run').click();
    await settle();
    expect($('status').textContent).toBe('Running in 2 views...');
    const runs = calls('run');
    expect(runs.map((call) => call.args)).toEqual([
      { name: 'Title', source: 'user', target: 1, remember: [1, 2] },
      { name: 'Title', source: 'user', target: 2, remember: [1, 2] },
    ]);
    for (const [index, call] of runs.entries()) {
      const view = index + 1;
      await reply(call, { views: [view], results: [{ ok: true, type: 'string', text: '"Home"', views: [view] }] });
    }
    const outcome = document.querySelector('#results [data-outcome]');
    expect(outcome?.querySelector('[data-text]')?.textContent).toBe('"Home"');
    expect(outcome?.querySelector('[data-views]')?.textContent).toBe('Views: Mobile, Desktop');
    expect(document.querySelector('#results [data-title]')?.textContent).toBe('Title');
  });

  it('runs changed code as written, in the active view only, and says a failure in words', async () => {
    await pick(0);
    await type(code(), 'return 42');
    (document.querySelector('#target [role="radio"][value="active"]') as HTMLElement).click();
    await settle();
    code().dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, bubbles: true }));
    await settle();
    expect(calls('run').map((call) => call.args)).toEqual([{ code: 'return 42', target: 2 }]);
    await reply(calls('run')[0], 'plugins.error.busy', false);
    const failed = document.querySelector('#results [data-outcome][data-ok="false"] [data-text]');
    expect(failed?.textContent).toBe('The plugin is busy.');
  });

  it('says when no view is active', async () => {
    await host({ type: 'event', message: { changed: true } });
    await reply(calls('list').at(-1), { ...LIST, active: null });
    await pick(0);
    (document.querySelector('#target [role="radio"][value="active"]') as HTMLElement).click();
    await settle();
    $('run').click();
    await settle();
    expect(calls('run')).toHaveLength(0);
    expect($('status').textContent).toBe('No view is active.');
    expect($('status').dataset.tone).toBe('error');
  });

  it('is used with the keyboard: Enter in the filter picks the first match', async () => {
    const filter = $('filter') as HTMLInputElement;
    filter.focus();
    await type(filter, 'title');
    expect(rows().map((row) => row.querySelector('[data-name]')?.textContent)).toEqual(['Title']);
    filter.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await settle();
    expect(($('name') as HTMLInputElement).value).toBe('Title');
    expect(document.activeElement).toBe(code());
  });

  it('moves through the list with the arrow keys and opens a snippet with Enter', async () => {
    const first = rows()[0] as HTMLElement;
    first.focus();
    first.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    await settle();
    expect(document.activeElement).toBe(rows()[1]);
    expect(rows()[1]?.getAttribute('tabindex')).toBe('0');
    expect(rows()[0]?.getAttribute('tabindex')).toBe('-1');
    (document.activeElement as HTMLElement).dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await settle();
    expect(($('name') as HTMLInputElement).value).toBe('Bad');
    expect(document.activeElement).toBe(code());
  });

  it('shows a refused save with its reason', async () => {
    await host({ type: 'event', message: { changed: true } });
    await reply(calls('list').at(-1), { ...LIST, local: 'loaded' });
    await type($('name') as HTMLInputElement, 'Mine');
    await type(code(), 'return 1');
    $('save').click();
    await settle();
    expect(calls('save')[0]?.args).toEqual({ name: 'Mine', code: 'return 1' });
    await reply(calls('save')[0], 'plugin.snippets.errors.save_untrusted', false);
    expect($('status').textContent).toContain('Saving needs trust.');
    expect($('status').dataset.tone).toBe('error');
    // RB-094-21: "keep the snippet in Settings > DevTools & Debugging > Snippets" links there.
    ($('status').querySelector('[data-testid="setting-link-snippets.library"]') as HTMLElement).click();
    expect(posted).toContainEqual({ target: 'host', action: 'open_settings', key: 'snippets.library' });
  });

  // RB-094-21: "or add one in Settings > DevTools & Debugging > Snippets" links there.
  it('links the empty list to the snippets of the settings', async () => {
    await type($('filter') as HTMLInputElement, 'no snippet has this name');
    ($('empty').querySelector('[data-testid="setting-link-snippets.library"]') as HTMLElement).click();
    expect(posted).toContainEqual({ target: 'host', action: 'open_settings', key: 'snippets.library' });
  });

  it('deletes a saved snippet only after asking, inline', async () => {
    const saved = { source: 'local' as const, name: 'Mine', description: '', world: 'isolated' as const, target: 'all' as const, timeoutMs: 5000, code: 'return 1' };
    await host({ type: 'event', message: { changed: true } });
    await reply(calls('list').at(-1), { ...LIST, local: 'loaded', snippets: [...LIST.snippets, saved] });
    await pick(2);
    $('delete').focus();
    $('delete').click();
    await settle();
    expect(calls('save')).toHaveLength(0);
    expect($('confirm-delete').textContent).toContain('Delete "Mine"?');
    // RB-090-23 (WCAG 2.4.3): the question takes the focus on Keep and gives it back to Delete.
    expect(document.activeElement).toBe($('keep'));
    $('keep').click();
    await settle();
    expect(document.getElementById('confirm-delete')).toBeNull();
    expect(document.activeElement).toBe($('delete'));
    $('delete').click();
    await settle();
    $('confirm').click();
    await settle();
    // Delete disables itself with the snippet gone: the name field takes the focus.
    expect(document.activeElement).toBe($('name'));
    expect(calls('save').map((call) => call.args)).toEqual([{ name: 'Mine', delete: true }]);
  });

  it('shows a run the command repeated and reloads after a change', async () => {
    await host({ type: 'event', message: { ran: { views: [1], snippet: { source: 'user', name: 'Title' },
      results: [{ ok: false, type: 'error', text: 'Error: x', views: [1], stack: 'at y', timedOut: true }] } } });
    expect(document.querySelector('#results [data-run] [data-title]')?.textContent).toBe('Title');
    expect(document.querySelector('#results [data-text]')?.textContent).toBe('Error: x');
    expect(document.querySelector('#results [data-timed-out]')).not.toBeNull();
    (document.querySelector('#results [data-stack-toggle]') as HTMLButtonElement).click();
    await settle();
    expect(document.querySelector('#results [data-stack]')?.textContent).toBe('at y');
    const before = calls('list').length;
    await host({ type: 'event', message: { changed: true } });
    expect(calls('list').length).toBeGreaterThan(before);
  });

  // RB-080-65: the editor, the results and their stacks in syntax colours.
  it('colours the code it edits, the results and their stacks', async () => {
    await pick(0);
    expect(code().parentElement?.className).toBe('hl-editor');
    expect(code().parentElement?.querySelector('.hl-backdrop .hl-keyword')?.textContent).toBe('return');
    $('run').click();
    await settle();
    const stack = 'TypeError: x\n    at f (https://a.test/a.js:1:2)';
    for (const [index, call] of calls('run').entries()) {
      const view = index + 1;
      await reply(call, { views: [view], results: [{ ok: index === 0, type: 'object', text: index === 0 ? '{"title":"<b>Home</b>"}' : 'TypeError: x', stack, views: [view] }] });
    }
    const ok = document.querySelector('#results [data-outcome][data-ok="true"] [data-text]');
    expect(ok?.querySelector('.hl-key')?.textContent).toBe('"title"');
    expect(ok?.querySelector('b')).toBeNull();
    for (const toggle of document.querySelectorAll<HTMLButtonElement>('#results [data-stack-toggle]')) toggle.click();
    await settle();
    expect(document.querySelector('#results [data-stack] .hl-fn')?.textContent).toBe('f');
  });

  // Review of RB-080-65: a string value is text, not JavaScript.
  it('shows a string result as text', async () => {
    await pick(0);
    $('run').click();
    await settle();
    for (const [index, call] of calls('run').entries()) {
      await reply(call, { views: [index + 1], results: [{ ok: true, type: 'string', text: '"don\'t do this"', views: [index + 1] }] });
    }
    const text = document.querySelector('#results [data-outcome] [data-text]');
    expect(text?.textContent).toBe('"don\'t do this"');
    expect(text?.querySelectorAll('[class^="hl-"]')).toHaveLength(0);
  });

  it('shows code in one colour when ui.syntaxHighlight is off', async () => {
    await host({ type: 'settings', strings: STRINGS, settings: { 'ui.syntaxHighlight': false } });
    await pick(0);
    expect((code().parentElement as HTMLElement).dataset.highlight).toBe('off');
  });

  it("takes the app's theme from its settings", async () => {
    await host({ type: 'settings', strings: STRINGS, settings: { 'ui.theme': 'dark' } });
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    await host({ type: 'settings', strings: STRINGS, settings: {} });
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it('ignores messages from anyone but the panels window', async () => {
    const before = posted.length;
    window.dispatchEvent(new MessageEvent('message', { data: { type: 'event', message: { changed: true } }, source: null }));
    await settle();
    expect(posted.length).toBe(before);
  });
});
