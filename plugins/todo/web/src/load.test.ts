// The todo panel's paged reads (RB-110-13, acceptance 2026-11-16): the heads page by page, then
// every file whose rev is new item page by page, a file that changes meanwhile from its start.
import { describe, expect, it } from 'vitest';
import { chunks, listHeads, loadFile, MAX_ARGS_BYTES, MAX_RESTARTS, readFiles, type ListCall } from './panel/load';
import type { TodoFile } from './panel/model';

const COUNTS = { open: 1, done: 0, cancelled: 0 };
const task = (line: number) => ({ line, depth: 0, kind: 'task', state: 'open', text: `task ${line}`, raw: `task ${line}`, tags: [], project: '', archived: false });

/** A component that answers `list` from `pages`: per file its item pages, and the heads. */
function component(files: Record<string, { rev: string; pages: number[][] }>, headsPerPage = 100) {
  const calls: Record<string, unknown>[] = [];
  const call: ListCall = async (args) => {
    calls.push(args);
    const from = typeof args.from === 'number' ? args.from : 0;
    if (typeof args.file === 'string') {
      const file = files[args.file];
      if (!file) throw Object.assign(new Error('plugins.error.not_found'), { params: { path: args.file } });
      if (args.rev !== undefined && args.rev !== file.rev) throw Object.assign(new Error('plugin.todo.errors.changed'), { params: { path: args.file } });
      const index = file.pages.findIndex((page) => page[0] === from + 1);
      const page = file.pages[index] ?? [];
      const answer: Record<string, unknown> = { path: args.file, rev: file.rev, counts: COUNTS, items: page.map(task) };
      if (index + 1 < file.pages.length) answer.next = (file.pages[index + 1]?.[0] ?? 1) - 1;
      return answer;
    }
    const paths = Array.isArray(args.files) ? (args.files as string[]) : Object.keys(files).sort();
    const heads = paths.slice(from, from + headsPerPage).map((path) => {
      const file = files[path];
      return file ? { path, rev: file.rev, counts: COUNTS } : { path, error: 'plugins.error.not_found', params: { path } };
    });
    const answer: Record<string, unknown> = { files: heads, truncated: false };
    if (from + headsPerPage < paths.length) answer.next = from + headsPerPage;
    return answer;
  };
  return { call, calls };
}

describe('the paged reads', () => {
  it('read a file of many pages whole, with the first page rev', async () => {
    const pages = Array.from({ length: 30 }, (_, page) => Array.from({ length: 100 }, (_, n) => page * 100 + n + 1));
    const { call, calls } = component({ 'big.todo': { rev: 'r1', pages } });
    const file = await loadFile(call, 'big.todo');
    expect(file.items).toHaveLength(3000);
    expect(file.items.at(-1)?.line).toBe(3000);
    expect(file.rev).toBe('r1');
    expect(calls).toHaveLength(30);
    expect(calls[0]).toEqual({ file: 'big.todo' });
    expect(calls[1]).toEqual({ file: 'big.todo', from: 100, rev: 'r1' });
  });

  it('go on from a page a change answered', async () => {
    const { call, calls } = component({ 'a.todo': { rev: 'r1', pages: [[1, 2], [3]] } });
    const first = { path: 'a.todo', rev: 'r1', counts: COUNTS, items: [task(1), task(2)], next: 2 };
    const file = await loadFile(call, 'a.todo', first);
    expect(file.items.map((item) => item.line)).toEqual([1, 2, 3]);
    expect(calls).toEqual([{ file: 'a.todo', from: 2, rev: 'r1' }]);
  });

  it('start a file again that changed between pages, and give up after a few times', async () => {
    const files = { 'a.todo': { rev: 'r1', pages: [[1], [2]] } };
    const { call, calls } = component(files);
    let changes = 1;
    const changing: ListCall = async (args) => {
      const answer = await call(args);
      // The file changes once, right after its first page was read.
      if (changes > 0 && args.from === undefined) {
        changes -= 1;
        files['a.todo'].rev = 'r2';
      }
      return answer;
    };
    const file = await loadFile(changing, 'a.todo');
    expect(file.rev).toBe('r2');
    expect(file.items.map((item) => item.line)).toEqual([1, 2]);
    expect(calls.map((args) => args.from ?? 0)).toEqual([0, 1, 0, 1]);

    let flips = 0;
    const always: ListCall = async (args) => {
      const answer = await call(args);
      if (args.from === undefined) files['a.todo'].rev = `r${(flips += 1) + 2}`;
      return answer;
    };
    await expect(loadFile(always, 'a.todo')).rejects.toThrow('plugin.todo.errors.changed');
    expect(flips).toBe(MAX_RESTARTS + 1);
  });

  it('show a file that does not read with its reason', async () => {
    const { call } = component({});
    const file = await loadFile(call, 'gone.todo');
    expect(file.error).toEqual({ code: 'plugins.error.not_found', params: { path: 'gone.todo' } });
    expect(file.items).toEqual([]);
  });

  it('never loop on a next that does not move on', async () => {
    const stuck: ListCall = async () => ({ path: 'a.todo', rev: 'r', counts: COUNTS, items: [task(1)], next: 0 });
    const file = await loadFile(stuck, 'a.todo');
    expect(file.items).toHaveLength(1);
  });

  it('list the heads page by page, the files named in groups that fit a message', async () => {
    const files = Object.fromEntries(Array.from({ length: 250 }, (_, n) => [`f${String(n).padStart(3, '0')}.todo`, { rev: `r${n}`, pages: [[1]] }]));
    const { call, calls } = component(files, 100);
    const all = await listHeads(call, null);
    expect(all.files).toHaveLength(250);
    expect(calls.map((args) => args.from ?? 0)).toEqual([0, 100, 200]);

    const long = Array.from({ length: 60 }, (_, n) => `${'d'.repeat(1000)}/${n}.todo`);
    const groups = chunks(long);
    expect(groups.length).toBeGreaterThan(1);
    expect(groups.flat()).toEqual(long);
    for (const group of groups) expect(new TextEncoder().encode(JSON.stringify(group)).length).toBeLessThanOrEqual(MAX_ARGS_BYTES);
  });

  it('read again only the files whose rev changed', async () => {
    const { call, calls } = component({ 'a.todo': { rev: 'r1', pages: [[1]] }, 'b.todo': { rev: 'r2', pages: [[1], [2]] } });
    const shown: TodoFile[] = [
      { path: 'a.todo', rev: 'r1', counts: COUNTS, items: [], error: null },
      { path: 'b.todo', rev: 'old', counts: COUNTS, items: [], error: null },
    ];
    const listing = await readFiles(call, shown, false);
    expect(calls).toEqual([{ files: ['a.todo', 'b.todo'] }, { file: 'b.todo' }, { file: 'b.todo', from: 1, rev: 'r2' }]);
    expect(listing.files[0]).toBe(shown[0]);
    expect(listing.files[1]?.items.map((item) => item.line)).toEqual([1, 2]);
  });
});
