// The todo panel's search field (RB-110-18) in a window of its own: Ctrl+F, the terms, the
// context, the marks, Esc, "only open", the section beside it and the search kept over a read.
// The panels window is played as in panel.test.ts.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';

type Posted = { type?: string; id?: number; tool?: string; args?: Record<string, unknown> };

const LOCALE = JSON.parse(readFileSync(join(process.cwd(), '..', 'locales', 'en.json'), 'utf8')) as {
  plugin: { todo: Record<string, unknown> };
};

function strings(): Record<string, string> {
  const flat: Record<string, string> = {};
  const walk = (value: unknown, prefix: string): void => {
    if (typeof value === 'string') flat[prefix] = value;
    else if (value && typeof value === 'object') for (const [key, child] of Object.entries(value)) walk(child, `${prefix}.${key}`);
  };
  walk(LOCALE.plugin.todo, 'plugin.todo');
  return flat;
}

const line = (n: number, kind: string, text: string, extra: Record<string, unknown> = {}) => ({
  line: n,
  depth: 0,
  kind,
  state: kind === 'task' ? 'open' : undefined,
  text,
  raw: text,
  tags: [],
  project: '',
  archived: false,
  ...extra,
});

/** Two Markdown sections, a project in the first, a done task, a note and the archive. */
const FILE = {
  path: 'TODO.todo',
  rev: 'aaaaaaaaaaaaaaaa',
  counts: { open: 3, done: 1, cancelled: 0 },
  items: [
    line(1, 'heading', 'Relaunch', { level: 1, raw: '# Relaunch' }),
    line(2, 'project', 'Website', { raw: 'Website:' }),
    line(3, 'task', 'Landing page for the café', { depth: 1, raw: 'Landing page for the café @high', tags: [{ name: 'high', value: null }], project: 'Website' }),
    line(4, 'task', 'Colours', { depth: 1, state: 'done', tags: [{ name: 'done', value: '26-11-14 09:30' }], project: 'Website' }),
    line(5, 'note', 'A note on the landing', { depth: 2, project: 'Website' }),
    line(6, 'heading', 'Copy', { level: 1, raw: '# Copy' }),
    line(7, 'task', 'Cafe menu text'),
    line(8, 'task', 'Footer'),
    line(10, 'project', 'Archive', { raw: 'Archive:' }),
    line(11, 'task', 'Old landing', { depth: 1, state: 'done', archived: true, project: 'Archive' }),
  ],
};

let posted: Posted[] = [];
let unmount: () => void = () => undefined;

const $ = (selector: string): HTMLElement | null => document.querySelector<HTMLElement>(selector);
const settle = async (): Promise<void> => {
  for (let round = 0; round < 3; round += 1) {
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
  }
};
async function fromHost(data: unknown): Promise<void> {
  window.dispatchEvent(new MessageEvent('message', { data, source: window }));
  await settle();
}
const lastCall = (): Posted | undefined => posted.filter((message) => message.type === 'call').at(-1);
const answer = (value: unknown): Promise<void> => fromHost({ type: 'result', id: lastCall()?.id, ok: true, value });

async function start(file: Record<string, unknown> = FILE): Promise<void> {
  await fromHost({ type: 'settings', strings: strings(), settings: { 'ui.theme': 'light', 'todo.refreshSeconds': 0 } });
  await answer({ files: [{ path: file.path, rev: file.rev, counts: file.counts }], truncated: false });
  await answer(file);
}

const input = (selector: string): HTMLInputElement =>
  (document.querySelector<HTMLInputElement>(`input${selector}`) ?? document.querySelector<HTMLInputElement>(`${selector} input`)) as HTMLInputElement;
const row = (n: number): HTMLElement | null => $(`[data-file="TODO.todo"] [data-line="${n}"]`);
const shownLines = (): number[] => [...document.querySelectorAll<HTMLElement>('[data-items] [data-line]')].map((shown) => Number(shown.dataset.line));

async function search(text: string): Promise<void> {
  const field = input('#search');
  field.value = text;
  field.dispatchEvent(new Event('input'));
  await settle();
}

beforeEach(() => {
  posted = [];
  document.body.innerHTML = '<div id="app"></div>';
  vi.spyOn(window, 'postMessage').mockImplementation((message: unknown) => {
    posted.push(message as Posted);
  });
  unmount = mountPanel(App, { prefix: 'plugin.todo.', name: 'todo panel' }).unmount;
});

afterEach(() => {
  unmount();
  vi.restoreAllMocks();
});

describe('the todo search field', () => {
  it('takes the caret on Ctrl+F, shows the matches with their headings and projects, marked', async () => {
    await start();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'f', ctrlKey: true, bubbles: true, cancelable: true }));
    await settle();
    expect(document.activeElement).toBe(input('#search'));
    await search('CAFE');
    expect(shownLines()).toEqual([1, 2, 3, 6, 7]);
    expect([...(row(3)?.querySelectorAll('[data-hit]') ?? [])].map((hit) => hit.textContent)).toEqual(['café']);
    expect(row(3)?.querySelector('[data-text]')?.textContent).toBe('Landing page for the café');
    expect(row(7)?.querySelector('[data-hit]')?.textContent).toBe('Cafe');
    // The context lines carry no mark.
    expect(row(2)?.querySelector('[data-hit]')).toBeNull();
    await search('@hi');
    expect(shownLines()).toEqual([1, 2, 3]);
    expect(row(3)?.querySelector('[data-tag]')?.hasAttribute('data-tag-hit')).toBe(true);
  });

  it('clears with Esc and with its button, and says when nothing matches', async () => {
    await start();
    await search('nothing like it');
    expect(shownLines()).toEqual([]);
    expect($('[data-no-match]')).not.toBeNull();
    input('#search').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    await settle();
    expect(input('#search').value).toBe('');
    expect(shownLines()).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);
    await search('footer');
    expect(shownLines()).toEqual([6, 8]);
    $('#search-clear')?.click();
    await settle();
    expect(input('#search').value).toBe('');
    expect($('[data-no-match]')).toBeNull();
  });

  it('shows the open todos alone with "only open", with the search or without', async () => {
    await start();
    $('#open-only')?.click();
    await settle();
    expect(shownLines()).toEqual([1, 2, 3, 6, 7, 8]);
    await search('landing');
    expect(shownLines()).toEqual([1, 2, 3]);
    // The archive's done todo is no open one.
    expect($('[data-archive-toggle]')).toBeNull();
  });

  it('applies together with the section, each cleared alone', async () => {
    await start();
    // A new heading is chosen as the section (RB-110-16).
    const field = input('[data-new]');
    field.value = '## Next';
    field.dispatchEvent(new Event('input'));
    await settle();
    field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    await settle();
    const after = {
      ...FILE,
      rev: 'bbbbbbbbbbbbbbbb',
      items: [...FILE.items.slice(0, 8), line(9, 'heading', 'Next', { level: 2, raw: '## Next' }), line(10, 'task', 'Café terrace'), ...FILE.items.slice(8).map((item) => ({ ...item, line: item.line + 2 }))],
    };
    await answer({ line: 9, file: after });
    expect(shownLines()).toEqual([9, 10]);
    await search('cafe');
    expect(shownLines()).toEqual([9, 10]);
    await search('menu');
    expect(shownLines()).toEqual([]);
    // The section cleared: the search goes on over the whole file.
    $('[data-section-clear]')?.click();
    await settle();
    expect(shownLines()).toEqual([6, 7]);
    expect(input('#search').value).toBe('menu');
  });

  it('keeps the search while the files are read again', async () => {
    await start();
    await search('footer');
    await fromHost({ type: 'event', message: { changed: 'TODO.todo' } });
    await answer({ files: [{ path: 'TODO.todo', rev: 'cccccccccccccccc', counts: FILE.counts }], truncated: false });
    await answer({ ...FILE, rev: 'cccccccccccccccc' });
    expect(input('#search').value).toBe('footer');
    expect(shownLines()).toEqual([6, 8]);
  });
});
