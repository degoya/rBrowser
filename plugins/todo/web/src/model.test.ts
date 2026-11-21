// The todo panel's reading of the component's answers (RB-110-13).
import { describe, expect, it } from 'vitest';
import { addTargets, isConflict, look, panelSettings, readFailures, readFile, readListing, sectionOf, tagNames, utcOffset, visible, type Item } from './panel/model';

const item = (line: number, extra: Partial<Item>): unknown => ({
  line,
  depth: 1,
  kind: 'task',
  state: 'open',
  text: `task ${line}`,
  raw: `task ${line}`,
  tags: [],
  project: 'Website',
  archived: false,
  ...extra,
});

const FILE = {
  path: 'TODO.todo',
  rev: '0123456789abcdef',
  counts: { open: 2, done: 1, cancelled: 0 },
  items: [
    item(1, { depth: 0, kind: 'project', state: undefined, text: 'Website', raw: 'Website:', project: '' }),
    item(2, { tags: [{ name: 'high', value: null }] }),
    item(3, { state: 'done', tags: [{ name: 'done', value: '26-11-16 14:05' }] }),
    item(4, { depth: 1, kind: 'project', state: undefined, text: 'Design', project: 'Website' }),
    item(5, { depth: 2, project: 'Website.Design', tags: [{ name: 'today', value: null }] }),
    item(7, { depth: 0, kind: 'project', state: undefined, text: 'Archive', raw: 'Archive:', project: '' }),
    item(8, { state: 'done', project: 'Archive', archived: true }),
  ],
};

describe('the todo answers', () => {
  it('take no array for an object: a tag that is a list is no tag (RB-094-10)', () => {
    const file = readFile({ ...FILE, items: [{ ...(item(2, {}) as object), tags: [['high'], { name: 'low', value: null }] }] });
    expect(file?.items[0]?.tags).toEqual([{ name: 'low', value: null }]);
    expect(readFile(['TODO.todo'])).toBeNull();
  });

  it('read files and refuse what is no file', () => {
    const listing = readListing({ files: [FILE, { path: 'bad.todo', error: 'plugin.todo.errors.encoding', params: { path: 'bad.todo' } }, 7], truncated: true });
    expect(listing.truncated).toBe(true);
    expect(listing.files.map((file) => file.path)).toEqual(['TODO.todo', 'bad.todo']);
    expect(listing.files[0]?.items).toHaveLength(7);
    expect(listing.files[0]?.items[0]?.state).toBeNull();
    expect(listing.files[1]?.error).toEqual({ code: 'plugin.todo.errors.encoding', params: { path: 'bad.todo' } });
    expect(readListing(null)).toEqual({ files: [], truncated: false });
    expect(readFile({ rev: 'x' })).toBeNull();
  });

  it('show the archive apart, finished todos hidden on request and one tag alone', () => {
    const file = readFile(FILE);
    if (!file) throw new Error('file');
    const all = visible(file.items, { hideDone: false, hideArchive: false, refreshSeconds: 5 }, null);
    expect(all.active.map((shown) => shown.line)).toEqual([1, 2, 3, 4, 5]);
    expect(all.archive.map((shown) => shown.line)).toEqual([8]);
    const left = visible(file.items, { hideDone: true, hideArchive: true, refreshSeconds: 5 }, null);
    expect(left.active.map((shown) => shown.line)).toEqual([1, 2, 4, 5]);
    expect(left.archive).toEqual([]);
    const today = visible(file.items, { hideDone: false, hideArchive: false, refreshSeconds: 5 }, 'today');
    expect(today.active.map((shown) => shown.line)).toEqual([5]);
    expect(tagNames([file])).toEqual(['high', 'today']);
    expect(addTargets(file)).toEqual([
      { line: 1, label: 'Website' },
      { line: 4, label: 'Website.Design' },
    ]);
  });

  it('take the settings with their defaults', () => {
    expect(panelSettings({})).toEqual({ hideDone: false, hideArchive: false, refreshSeconds: 5 });
    expect(panelSettings({ 'todo.hideDone': true, 'todo.hideArchive': true, 'todo.refreshSeconds': 0 })).toEqual({
      hideDone: true,
      hideArchive: true,
      refreshSeconds: 0,
    });
    expect(panelSettings({ 'todo.refreshSeconds': 9999 }).refreshSeconds).toBe(5);
  });

  it('know a conflict and the local offset', () => {
    expect(isConflict(new Error('plugins.error.write_conflict'))).toBe(true);
    expect(isConflict(new Error('plugin.todo.errors.changed'))).toBe(true);
    expect(isConflict(new Error('plugin.todo.errors.untrusted'))).toBe(false);
    expect(isConflict('plugins.error.write_conflict')).toBe(false);
    expect(utcOffset({ getTimezoneOffset: () => -60 } as Date)).toBe(60);
  });

  it('read the failures of an archive as errors with their parameters', () => {
    const failed = readFailures([{ path: 'a.todo', error: 'plugins.error.write_conflict', params: { path: 'a.todo' } }, { path: 'b.todo' }, 3]);
    expect(failed).toHaveLength(1);
    expect(isConflict(failed[0])).toBe(true);
    expect((failed[0] as Error & { params: Record<string, string> }).params).toEqual({ path: 'a.todo' });
    expect(readFailures(undefined)).toEqual([]);
  });

  it('read Markdown headings with their level, offer them as places for a new todo and make them stand out', () => {
    const file = readFile({
      ...FILE,
      items: [
        item(1, { depth: 0, kind: 'heading', state: undefined, level: 2, text: 'Ratios / Images', raw: '## Ratios / Images', project: '' }),
        item(2, { depth: 0, project: '' }),
        item(3, { depth: 0, kind: 'heading', state: undefined, level: 9, text: 'Deep', raw: '###### Deep', project: '' }),
        item(4, { depth: 0, kind: 'project', state: undefined, text: 'Website', raw: 'Website:', project: '' }),
        item(5, { depth: 1, kind: 'project', state: undefined, text: 'Design', raw: 'Design:', project: 'Website' }),
        item(6, { depth: 1, kind: 'note', state: undefined, text: 'A note', raw: 'A note' }),
      ],
    });
    if (!file) throw new Error('file');
    const [heading, task, deep, project, nested, note] = file.items as [Item, Item, Item, Item, Item, Item];
    expect(heading).toMatchObject({ kind: 'heading', level: 2, state: null, text: 'Ratios / Images', raw: '## Ratios / Images' });
    expect(deep.level).toBe(6);
    expect(task.level).toBeNull();
    expect(addTargets(file)).toEqual([
      { line: 1, label: '## Ratios / Images' },
      { line: 3, label: '###### Deep' },
      { line: 4, label: 'Website' },
      { line: 5, label: 'Website.Design' },
    ]);
    // Bold, larger by level, a heading's level for assistive technology below the file's own h2.
    expect(look(heading).ariaLevel).toBe(4);
    expect(look(heading).text).toContain('font-bold');
    expect(look({ ...heading, level: 1 }).text).toContain('text-2xl');
    expect(look({ ...heading, level: 1 }).text).not.toBe(look(heading).text);
    expect(look(deep).ariaLevel).toBe(6);
    // A project is a heading too, the top-level ones larger than the nested.
    expect(look(project).ariaLevel).toBe(3);
    expect(look(project).text).toContain('font-bold');
    expect(look(nested).ariaLevel).toBe(4);
    expect(look(nested).text).not.toBe(look(project).text);
    // Notes and tasks are no headings.
    expect(look(note).ariaLevel).toBeNull();
    expect(look(note).text).toContain('italic');
    expect(look(task).ariaLevel).toBeNull();
  });

  it('give every heading level a size, weight and room of its own (RB-110-16)', () => {
    const heading = (level: number): Item => ({ ...(readFile({ ...FILE, items: [item(1, { depth: 0, kind: 'heading', level })] })?.items[0] as Item) });
    const looks = [1, 2, 3, 4, 5, 6].map((level) => look(heading(level)));
    const size = (text: string): string => /\btext-(xs|sm|base|lg|xl|2xl)\b/.exec(text)?.[1] ?? '';
    expect(new Set(looks.map((shape) => size(shape.text))).size).toBe(6);
    expect(looks.map((shape) => size(shape.text))).toEqual(['2xl', 'xl', 'lg', 'base', 'sm', 'xs']);
    expect(new Set(looks.map((shape) => shape.row)).size).toBe(6);
    // A task is text-sm: every heading above level 5 is larger than a task.
    expect(size(look(heading(4)).text)).not.toBe('sm');
  });

  it('cut a file to the section of a heading, project or task (RB-110-16)', () => {
    const heading = (line: number, level: number, depth = 0) => item(line, { depth, kind: 'heading', state: undefined, level, project: '' });
    const project = (line: number, depth: number, text: string) => item(line, { depth, kind: 'project', state: undefined, text, raw: `${text}:` });
    const file = readFile({
      ...FILE,
      items: [
        heading(1, 1),
        item(2, { depth: 0 }),
        heading(3, 2),
        item(4, { depth: 0 }),
        heading(5, 2),
        heading(6, 3),
        item(7, { depth: 0 }),
        heading(8, 1),
        item(9, { depth: 0 }),
        project(10, 0, 'Website'),
        item(11, { depth: 1 }),
        project(12, 1, 'Design'),
        item(13, { depth: 2 }),
        item(14, { depth: 1 }),
        project(15, 0, 'Archive'),
        item(16, { state: 'done', project: 'Archive', archived: true }),
      ],
    });
    if (!file) throw new Error('file');
    const lines = (line: number): number[] | null => sectionOf(file.items, line)?.map((shown) => shown.line) ?? null;
    expect(lines(3)).toEqual([3, 4]);
    expect(lines(5)).toEqual([5, 6, 7]);
    expect(lines(1)).toEqual([1, 2, 3, 4, 5, 6, 7]);
    expect(lines(8)).toEqual([8, 9, 10, 11, 12, 13, 14]);
    expect(lines(10)).toEqual([10, 11, 12, 13, 14]);
    expect(lines(12)).toEqual([12, 13]);
    expect(lines(0)).toBeNull();
    expect(lines(99)).toBeNull();
    // The list shows the section alone, the archive not; the top level (0) everything.
    const settings = panelSettings({});
    expect(visible(file.items, settings, null, 12).active.map((shown) => shown.line)).toEqual([12, 13]);
    expect(visible(file.items, settings, null, 12).archive).toEqual([]);
    expect(visible(file.items, settings, null, 0).archive).toHaveLength(1);
  });
});
