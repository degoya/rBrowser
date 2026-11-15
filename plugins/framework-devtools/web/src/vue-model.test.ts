import { describe, expect, it } from 'vitest';
import type { TreeNode } from './vue-model';
import { allIds, formatValue, idsToDepth, matching, prune, rows, stateRows } from './vue-model';

const tree: TreeNode[] = [
  {
    id: 'a',
    label: 'App',
    children: [
      { id: 'b', name: 'Header', tags: [{ label: 'fragment' }] },
      { id: 'c', label: 'Main', children: [{ id: 'd', label: 'Card' }] },
    ],
  },
  { id: 'e', label: 'Footer' },
];

describe('rows', () => {
  it('shows the open nodes depth first with their tags', () => {
    expect(rows(tree, new Set(['a']))).toEqual([
      { id: 'a', label: 'App', depth: 0, hasChildren: true, expanded: true, tags: [] },
      { id: 'b', label: 'Header', depth: 1, hasChildren: false, expanded: false, tags: ['fragment'] },
      { id: 'c', label: 'Main', depth: 1, hasChildren: true, expanded: false, tags: [] },
      { id: 'e', label: 'Footer', depth: 0, hasChildren: false, expanded: false, tags: [] },
    ]);
    expect(rows(tree, new Set(['a', 'c'])).map((row) => row.id)).toEqual(['a', 'b', 'c', 'd', 'e']);
    expect(rows(tree, new Set()).map((row) => row.id)).toEqual(['a', 'e']);
  });

  it('names every id and the ids open to a depth', () => {
    expect(allIds(tree)).toEqual(['a', 'b', 'c', 'd', 'e']);
    expect(idsToDepth(tree, 1)).toEqual(['a']);
    expect(idsToDepth(tree, 2)).toEqual(['a', 'c']);
    expect(idsToDepth(tree, 0)).toEqual([]);
  });
});

describe('formatValue', () => {
  it('shows primitives, collections and custom values on one line', () => {
    expect(formatValue('x')).toBe('"x"');
    expect(formatValue(3)).toBe('3');
    expect(formatValue(true)).toBe('true');
    expect(formatValue(null)).toBe('null');
    expect(formatValue(undefined)).toBe('undefined');
    expect(formatValue([1, 2, 3])).toBe('Array(3)');
    expect(formatValue({})).toBe('{}');
    expect(formatValue({ a: 1, b: 2 })).toBe('Object {a, b}');
    expect(formatValue({ _custom: { displayText: 'Ref<3>' } })).toBe('Ref<3>');
    expect(formatValue({ _custom: { display: 'Date' } })).toBe('Date');
    expect(formatValue({ _custom: { value: 'inner' } })).toBe('"inner"');
    expect(formatValue({ _custom: { type: 'map' } })).toBe('map');
    expect(formatValue(() => 1)).toBe('function');
  });

  it('clips a long text', () => {
    const long = formatValue('y'.repeat(500));
    expect(long.length).toBe(200);
    expect(long.endsWith('…')).toBe(true);
  });
});

describe('stateRows', () => {
  it('flattens the sections and skips what is no entry', () => {
    const state = {
      props: [{ key: 'title', value: 'Hi', type: 'props' }, 'junk'],
      setup: [{ key: 'count', value: { _custom: { displayText: 'Ref<2>' } } }],
      other: 'no',
    };
    expect(stateRows(state)).toEqual([
      { section: 'props', key: 'title', value: '"Hi"' },
      { section: 'setup', key: 'count', value: 'Ref<2>' },
    ]);
    expect(stateRows(null)).toEqual([]);
  });
});

describe('matching and prune', () => {
  it('keeps the matches with their ancestors, everything for an empty filter', () => {
    expect([...matching(tree, '')]).toEqual(['a', 'b', 'c', 'd', 'e']);
    expect([...matching(tree, 'card').values()].sort()).toEqual(['a', 'c', 'd']);
    expect([...matching(tree, 'FOOT')]).toEqual(['e']);
    expect(matching(tree, 'zzz').size).toBe(0);
  });

  it('prunes the tree to the kept ids', () => {
    const kept = prune(tree, matching(tree, 'card'));
    expect(kept).toEqual([{ id: 'a', label: 'App', children: [{ id: 'c', label: 'Main', children: [{ id: 'd', label: 'Card' }] }] }]);
    expect(prune(tree, new Set())).toEqual([]);
  });
});
