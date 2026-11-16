// The panel's model: failures in words, the list's filter, the arguments of a run and the joined
// results.
import { describe, expect, it } from 'vitest';
import * as M from './panel/model';
import type { RunAnswer, SnippetEntry } from './panel/model';

const entry = (source: SnippetEntry['source'], name: string, description = '', code = 'return 1'): SnippetEntry => ({
  source, name, description, world: 'isolated', target: 'all', timeoutMs: 5000, code,
});

describe('the snippets model', () => {
  it('says a failure code in its sentence, never the code, and the page\'s own error as it came', () => {
    const strings = { 'plugin.snippets.errors.not_found': 'There is no snippet "{name}".', 'plugins.error.busy': 'The plugin is busy.' };
    expect(M.failureText(strings, 'plugin.snippets.errors.not_found', { name: 'a' })).toBe('There is no snippet "a".');
    expect(M.failureText(strings, 'plugin.snippets.errors.not_found name=b')).toBe('There is no snippet "b".');
    expect(M.failureText(strings, 'plugins.error.busy')).toBe('The plugin is busy.');
    expect(M.failureText(strings, 'plugins.error.not_available')).not.toContain('plugins.error');
    expect(M.failureText(strings, 'TypeError: x is not a function')).toBe('TypeError: x is not a function');
  });

  it('filters by every word and keeps the sources in order', () => {
    const list = [entry('local', 'Grid saved'), entry('project', 'Layout grid', 'columns'), entry('user', 'Title')];
    expect(M.filter(list, '').map((e) => e.source)).toEqual(['user', 'project', 'local']);
    expect(M.filter(list, 'GRID').map((e) => e.name)).toEqual(['Layout grid', 'Grid saved']);
    expect(M.filter(list, 'grid columns').map((e) => e.name)).toEqual(['Layout grid']);
  });

  it('runs in every view or the active one', () => {
    const views = [{ id: 1, label: 'A', breakpoint: 'a', engine: 'chromium' }, { id: 3, label: 'B', breakpoint: 'b', engine: 'chromium' }];
    expect(M.targetViews('all', views, 3)).toEqual([1, 3]);
    expect(M.targetViews('active', views, 3)).toEqual([3]);
    expect(M.targetViews('active', views, null)).toEqual([]);
  });

  it('runs a stored snippet by name and changed code as written', () => {
    const stored = entry('project', 'Grid');
    expect(M.runArgs(stored, 'return 1', 2, [1, 2])).toEqual({ name: 'Grid', source: 'project', target: 2, remember: [1, 2] });
    expect(M.runArgs(stored, 'return 2', 2, [1, 2])).toEqual({ code: 'return 2', target: 2 });
    expect(M.runArgs(null, 'return 3', 1, [1])).toEqual({ code: 'return 3', target: 1 });
  });

  it('joins the views answers into one result per outcome', () => {
    const ok = (view: number, text: string): RunAnswer => ({ views: [view], results: [{ ok: true, type: 'string', text, views: [view] }] });
    const results = M.join([
      { view: 1, answer: ok(1, '"Home"') },
      { view: 2, answer: ok(2, '"Home"') },
      { view: 3, answer: ok(3, '"Other"') },
      { view: 4, error: 'plugins.error.busy' },
    ]);
    expect(results.map((r) => [r.text, r.views])).toEqual([
      ['"Home"', [1, 2]],
      ['"Other"', [3]],
      ['plugins.error.busy', [4]],
    ]);
    expect(results[2]?.ok).toBe(false);
  });

  it('moves the focus with the arrow keys, Home and End', () => {
    expect(M.moveFocus(0, 'ArrowDown', 3)).toBe(1);
    expect(M.moveFocus(2, 'ArrowDown', 3)).toBe(2);
    expect(M.moveFocus(1, 'ArrowUp', 3)).toBe(0);
    expect(M.moveFocus(1, 'End', 3)).toBe(2);
    expect(M.moveFocus(1, 'Home', 3)).toBe(0);
    expect(M.moveFocus(1, 'x', 3)).toBeNull();
    expect(M.moveFocus(0, 'ArrowDown', 0)).toBeNull();
  });
});
