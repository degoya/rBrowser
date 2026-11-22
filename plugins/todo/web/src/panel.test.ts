// The todo panel in a window of its own (RB-110-13): the calls it sends and how it shows the
// answers. The panels window is played by spying on postMessage and dispatching `message` events
// from window.parent (the test window itself).
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { mountPanel } from '../../../web-common/panel-ui/src/mount';
import App from './panel/App.vue';

type Posted = { type?: string; id?: number; tool?: string; args?: Record<string, unknown>; dirty?: boolean };

// The tests run in plugins/todo/web (`npm test`).
const LOCALE = JSON.parse(readFileSync(join(process.cwd(), '..', 'locales', 'en.json'), 'utf8')) as {
  plugin: { todo: Record<string, unknown> };
};

/** The plugin's texts, flat, as the host sends them. */
function strings(): Record<string, string> {
  const flat: Record<string, string> = {
    'plugins.error.write_conflict': '{path} changed on disk since the plugin read it; nothing was written.',
    'plugins.error.busy': 'The plugin is busy.',
  };
  const walk = (value: unknown, prefix: string): void => {
    if (typeof value === 'string') flat[prefix] = value;
    else if (value && typeof value === 'object') for (const [key, child] of Object.entries(value)) walk(child, `${prefix}.${key}`);
  };
  walk(LOCALE.plugin.todo, 'plugin.todo');
  return flat;
}

const task = (line: number, text: string, extra: Record<string, unknown> = {}) => ({
  line,
  depth: 1,
  kind: 'task',
  state: 'open',
  text,
  raw: text,
  tags: [],
  project: 'Website',
  archived: false,
  ...extra,
});

const FILE = {
  path: 'TODO.todo',
  rev: 'aaaaaaaaaaaaaaaa',
  counts: { open: 2, done: 1, cancelled: 0 },
  items: [
    { line: 1, depth: 0, kind: 'project', text: 'Website', raw: 'Website:', tags: [], project: '', archived: false },
    task(2, 'Landing page', { raw: 'Landing page @high', tags: [{ name: 'high', value: null }] }),
    task(3, 'Colours', { state: 'done', raw: 'Colours @done(26-11-14 09:30)', tags: [{ name: 'done', value: '26-11-14 09:30' }] }),
    { line: 4, depth: 2, kind: 'note', text: 'A note on the colours', raw: 'A note on the colours', tags: [], project: 'Website', archived: false },
    task(5, 'Footer'),
    { line: 7, depth: 0, kind: 'project', text: 'Archive', raw: 'Archive:', tags: [], project: '', archived: false },
    task(8, 'Beta', { state: 'done', project: 'Archive', archived: true }),
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

const calls = (): Posted[] => posted.filter((message) => message.type === 'call');
const lastCall = (): Posted | undefined => calls().at(-1);

async function answer(value: unknown): Promise<void> {
  await fromHost({ type: 'result', id: lastCall()?.id, ok: true, value });
}

/** What the file list answers of a file: its head, no items. */
const headOf = (file: Record<string, unknown>): Record<string, unknown> =>
  'error' in file ? file : { path: file.path, rev: file.rev, counts: file.counts };

/** The panel's first read: the heads, then each file's items (one page each here). */
async function start(settings: Record<string, unknown> = {}, files: Record<string, unknown>[] = [FILE]): Promise<void> {
  await fromHost({ type: 'settings', strings: strings(), settings: { 'ui.theme': 'light', 'todo.refreshSeconds': 0, ...settings } });
  expect(lastCall()).toMatchObject({ tool: 'list', args: {} });
  await answer({ files: files.map(headOf), truncated: false });
  for (const file of files.filter((file) => !('error' in file))) {
    expect(lastCall()).toMatchObject({ tool: 'list', args: { file: file.path } });
    await answer(file);
  }
}

/** A UInput's input element, whichever element took the attribute. */
const input = (root: ParentNode, selector: string): HTMLInputElement =>
  (root.querySelector<HTMLInputElement>(`input${selector}`) ?? root.querySelector<HTMLInputElement>(`${selector} input`)) as HTMLInputElement;

const row = (line: number): HTMLElement | null => $(`[data-file="TODO.todo"] [data-line="${line}"]`);

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

describe('the todo panel', () => {
  it('shows each file with its projects, todos, notes, tags and the archive folded', async () => {
    await start();
    expect($('#title')?.textContent).toBe('Todo');
    expect(row(1)?.dataset.kind).toBe('project');
    expect(row(2)?.querySelector('[data-text]')?.textContent).toBe('Landing page');
    expect(row(2)?.querySelector('[data-tag]')?.textContent).toBe('@high');
    expect(row(3)?.dataset.state).toBe('done');
    expect(row(4)?.dataset.kind).toBe('note');
    // The Archive project is no row of its own; its todos are behind the fold.
    expect(row(7)).toBeNull();
    expect($('[data-archive-toggle]')?.textContent).toContain('Archive: 1 todo');
    expect($('[data-counts]')?.textContent).toBe('2 open, 1 done');
    // RB-110-16: the window keeps the frame, so a switch back does not search again.
    expect(posted).toContainEqual({ type: 'keep' });
  });

  it('checks a todo with the version shown and the local offset, and shows the answer', async () => {
    await start();
    row(2)?.querySelector<HTMLElement>('[data-check]')?.click();
    await settle();
    expect(lastCall()).toMatchObject({
      tool: 'toggle',
      args: { file: 'TODO.todo', rev: 'aaaaaaaaaaaaaaaa', line: 2, utcOffset: 0 - new Date().getTimezoneOffset() },
    });
    const done = { ...FILE, rev: 'bbbbbbbbbbbbbbbb', items: FILE.items.map((item) => (item.line === 2 ? { ...item, state: 'done' } : item)) };
    await answer({ line: 2, state: 'done', file: done });
    expect(row(2)?.dataset.state).toBe('done');
  });

  it('reads the files again when one changed elsewhere, and says so', async () => {
    await start();
    row(5)?.querySelector<HTMLElement>('[data-check]')?.click();
    await settle();
    await fromHost({ type: 'result', id: lastCall()?.id, ok: false, error: 'plugin.todo.errors.changed', params: { path: 'TODO.todo' } });
    expect(lastCall()).toMatchObject({ tool: 'list', args: {} });
    expect($('#status')?.dataset.tone).toBe('warn');
    expect($('#status')?.textContent).toContain('A file changed elsewhere');
    await answer({ files: [FILE], truncated: false });
    expect($('#status')?.dataset.tone).toBe('warn');
  });

  it('adds a todo with Enter and edits one in place', async () => {
    await start();
    const field = input(document, '[data-new]');
    field.value = 'Imprint';
    field.dispatchEvent(new Event('input'));
    await settle();
    field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    await settle();
    expect(lastCall()).toMatchObject({ tool: 'add', args: { file: 'TODO.todo', text: 'Imprint' } });
    await answer({ line: 6, file: FILE });

    row(5)?.querySelector<HTMLElement>('[data-edit-start]')?.click();
    await settle();
    expect(posted).toContainEqual({ type: 'dirty', dirty: true });
    const edit = input(row(5) as HTMLElement, '[data-edit]');
    expect(edit.value).toBe('Footer');
    edit.value = 'Footer links';
    edit.dispatchEvent(new Event('input'));
    await settle();
    edit.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    await settle();
    expect(lastCall()).toMatchObject({ tool: 'edit', args: { file: 'TODO.todo', line: 5, action: 'text', text: 'Footer links' } });
    expect(posted).toContainEqual({ type: 'dirty', dirty: false });
  });

  it('asks before it deletes, naming what goes with it', async () => {
    await start();
    row(3)?.querySelector<HTMLElement>('[data-delete]')?.click();
    await settle();
    expect(row(3)?.querySelector('[data-confirm]')?.textContent).toContain('Delete "Colours" and the 1 line below it?');
    expect(calls().filter((call) => call.tool === 'edit')).toEqual([]);
    row(3)?.querySelector<HTMLElement>('[data-confirm-delete]')?.click();
    await settle();
    expect(lastCall()).toMatchObject({ tool: 'edit', args: { line: 3, action: 'delete' } });
  });

  it('hides the finished todos and the archive on request and filters by tag', async () => {
    await start({ 'todo.hideDone': true, 'todo.hideArchive': true });
    expect(row(3)).toBeNull();
    expect($('[data-archive-toggle]')).toBeNull();
    expect(row(2)).not.toBeNull();
    expect($('#tag')).not.toBeNull();
  });

  it('starts TODO.todo in a project without todo files', async () => {
    await start({}, []);
    expect($('#empty')?.textContent).toContain('No .todo files');
    const field = input(document, '[data-new]');
    field.value = 'First';
    field.dispatchEvent(new Event('input'));
    await settle();
    $('[data-add]')?.click();
    await settle();
    expect(lastCall()).toMatchObject({ tool: 'add', args: { file: 'TODO.todo', text: 'First' } });
    expect(lastCall()?.args).not.toHaveProperty('rev');
  });

  it('says why a file does not read, never the code', async () => {
    await start({}, [{ path: 'bad.todo', error: 'plugin.todo.errors.encoding', params: { path: 'bad.todo' } }]);
    const text = $('[data-file="bad.todo"] [data-error]')?.textContent ?? '';
    expect(text).toContain('bad.todo is not UTF-8 text');
    expect(text).not.toContain('plugin.todo');
  });

  it('archives and says which file it could not, reading again one that changed elsewhere', async () => {
    await start();
    $('#archive')?.click();
    await settle();
    expect(lastCall()).toMatchObject({ tool: 'archive', args: {} });
    await answer({
      archived: 2,
      files: [headOf(FILE)],
      failed: [{ path: 'docs/plan.todo', error: 'plugin.todo.errors.encoding', params: { path: 'docs/plan.todo' } }],
      omitted: 0,
    });
    // The archived file's items are read again.
    expect(lastCall()).toMatchObject({ tool: 'list', args: { file: 'TODO.todo' } });
    await answer(FILE);
    expect($('#status')?.dataset.tone).toBe('error');
    expect($('#status')?.textContent).toContain('2 todos archived');
    expect($('#status')?.textContent).toContain('docs/plan.todo is not UTF-8 text');
    expect($('#status')?.textContent).not.toContain('plugin.todo');

    $('#archive')?.click();
    await settle();
    await answer({
      archived: 0,
      files: [],
      failed: [{ path: 'TODO.todo', error: 'plugins.error.write_conflict', params: { path: 'TODO.todo' } }],
    });
    expect(lastCall()).toMatchObject({ tool: 'list', args: {} });
    expect($('#status')?.dataset.tone).toBe('warn');
    expect($('#status')?.textContent).toContain('A file changed elsewhere');
  });

  it('reads a long file page by page and shows a cut line without offering to edit it', async () => {
    const first = { ...FILE, items: FILE.items.slice(0, 4), next: 4 };
    await start({}, [first]);
    expect(lastCall()).toMatchObject({ tool: 'list', args: { file: 'TODO.todo', from: 4, rev: 'aaaaaaaaaaaaaaaa' } });
    await answer({ ...FILE, items: [...FILE.items.slice(4, 6), task(9, 'Very long', { cut: true })] });
    expect(row(2)).not.toBeNull();
    expect(row(5)).not.toBeNull();
    expect(row(9)?.querySelector('[data-text]')?.textContent).toBe('Very long\u2026');
    expect(row(9)?.querySelector<HTMLButtonElement>('[data-edit-start]')?.disabled).toBe(true);
    expect(row(5)?.querySelector<HTMLButtonElement>('[data-edit-start]')?.disabled).toBe(false);
  });

  it('starts a file again that changed while its pages were read', async () => {
    await start({}, [{ ...FILE, items: FILE.items.slice(0, 4), next: 4 }]);
    await fromHost({ type: 'result', id: lastCall()?.id, ok: false, error: 'plugin.todo.errors.changed', params: { path: 'TODO.todo' } });
    expect(lastCall()).toMatchObject({ tool: 'list', args: { file: 'TODO.todo' } });
    expect(lastCall()?.args).not.toHaveProperty('from');
    await answer({ ...FILE, rev: 'cccccccccccccccc' });
    expect(row(5)).not.toBeNull();
    $('#archive')?.click();
    await settle();
    expect(lastCall()).toMatchObject({ tool: 'archive' });
  });

  it('looks at the heads and reads again only a file whose rev changed', async () => {
    await start();
    await fromHost({ type: 'event', message: { changed: 'TODO.todo' } });
    expect(lastCall()).toMatchObject({ tool: 'list', args: { files: ['TODO.todo'] } });
    const before = calls().length;
    await answer({ files: [headOf(FILE)], truncated: false });
    expect(calls()).toHaveLength(before);
    await fromHost({ type: 'event', message: { changed: 'TODO.todo' } });
    await answer({ files: [{ ...headOf(FILE), rev: 'dddddddddddddddd' }], truncated: false });
    expect(lastCall()).toMatchObject({ tool: 'list', args: { file: 'TODO.todo' } });
  });

  it('reads the shown files again when the component says one changed', async () => {
    await start();
    await fromHost({ type: 'event', message: { changed: 'TODO.todo' } });
    expect(lastCall()).toMatchObject({ tool: 'list', args: { files: ['TODO.todo'] } });
  });

  it('shows Markdown headings and projects as headings and edits a heading as its whole line', async () => {
    const heading = (line: number, level: number, text: string) => ({
      line,
      depth: 0,
      kind: 'heading',
      level,
      text,
      raw: `${'#'.repeat(level)} ${text}`,
      tags: [],
      project: '',
      archived: false,
    });
    const markdown = {
      ...FILE,
      items: [
        heading(1, 1, 'Relaunch'),
        heading(3, 2, 'Ratios / Images'),
        task(4, 'Hero image', { depth: 0, project: '' }),
        { line: 5, depth: 0, kind: 'project', text: 'Website', raw: 'Website:', tags: [], project: '', archived: false },
      ],
    };
    await start({}, [markdown]);
    const shown = row(3);
    expect(shown?.dataset.kind).toBe('heading');
    expect(shown?.dataset.level).toBe('2');
    const title = shown?.querySelector<HTMLElement>('[data-text]');
    expect(title?.textContent).toBe('Ratios / Images');
    expect(title?.getAttribute('role')).toBe('heading');
    expect(title?.getAttribute('aria-level')).toBe('4');
    expect(title?.className).toContain('font-bold');
    expect(title?.className).not.toContain('italic');
    expect(row(1)?.querySelector('[data-text]')?.className).not.toBe(title?.className);
    expect(shown?.querySelector('[data-check]')).toBeNull();
    expect(shown?.querySelector('[data-cancel]')).toBeNull();
    // A project stands out as a heading as well.
    const project = row(5)?.querySelector<HTMLElement>('[data-text]');
    expect(project?.getAttribute('role')).toBe('heading');
    expect(project?.className).toContain('font-bold');
    // A new todo may go below a heading.
    expect($('[data-target]')).not.toBeNull();

    shown?.querySelector<HTMLElement>('[data-edit-start]')?.click();
    await settle();
    const edit = input(shown as HTMLElement, '[data-edit]');
    expect(edit.value).toBe('## Ratios / Images');
    edit.value = '### Ratios and images';
    edit.dispatchEvent(new Event('input'));
    await settle();
    edit.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    await settle();
    expect(lastCall()).toMatchObject({ tool: 'edit', args: { line: 3, action: 'text', text: '### Ratios and images' } });
    await answer({ file: markdown });

    row(3)?.querySelector<HTMLElement>('[data-delete]')?.click();
    await settle();
    expect(row(3)?.querySelector('[data-confirm]')?.textContent).toContain('Delete "Ratios / Images"? The rest of the file stays.');
  });

  it('adds a heading written with #, chooses it and shows its section alone until cleared', async () => {
    const heading = (line: number, level: number, text: string) => ({
      line,
      depth: 0,
      kind: 'heading',
      level,
      text,
      raw: `${'#'.repeat(level)} ${text}`,
      tags: [],
      project: '',
      archived: false,
    });
    const markdown = { ...FILE, items: [heading(1, 1, 'Relaunch'), task(2, 'Hero image', { depth: 0, project: '' })] };
    await start({}, [markdown]);
    const type = async (text: string): Promise<void> => {
      const field = input(document, '[data-new]');
      field.value = text;
      field.dispatchEvent(new Event('input'));
      await settle();
      field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
      await settle();
    };
    await type('## Sprint');
    expect(lastCall()).toMatchObject({ tool: 'add', args: { file: 'TODO.todo', text: '## Sprint' } });
    expect(lastCall()?.args).not.toHaveProperty('under');
    const after = { ...markdown, rev: 'bbbbbbbbbbbbbbbb', items: [...markdown.items, heading(4, 2, 'Sprint')] };
    await answer({ line: 4, file: after });
    // The new heading is chosen: the list shows its section alone, the filter says so.
    expect(row(4)?.dataset.kind).toBe('heading');
    expect(row(2)).toBeNull();
    expect($('[data-section-clear]')?.textContent).toContain('## Sprint');
    await type('Plan it');
    expect(lastCall()).toMatchObject({ tool: 'add', args: { text: 'Plan it', under: 4, rev: 'bbbbbbbbbbbbbbbb' } });
    await answer({ line: 5, file: after });
    $('[data-section-clear]')?.click();
    await settle();
    expect(row(2)).not.toBeNull();
    expect($('[data-section-clear]')).toBeNull();
  });
});
