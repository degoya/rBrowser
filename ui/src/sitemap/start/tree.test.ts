import { describe, expect, it } from 'vitest';
import type { SitemapTreeNode } from '../../ipc/protocol';
import {
  Selection,
  captureArgs,
  checkState,
  countsOf,
  flatten,
  isPage,
  nodeCounts,
  search,
  toggle,
  visibleRows,
  windowOf,
  type PageTree,
} from './tree';

const leaf = (name: string, first: number): SitemapTreeNode => ({ name, first, count: 1 });

/** `https://a.test` > `/`, `about`, `de` > (`/de`, `produkte` > (`a`, `b`)). */
function sample(): PageTree {
  const pages = ['/', '/about', '/de', '/de/produkte/a', '/de/produkte/b'].map((path) => `https://a.test${path}`);
  const tree: SitemapTreeNode[] = [
    {
      name: 'https://a.test',
      first: 0,
      count: 5,
      children: [
        leaf('/', 0),
        leaf('about', 1),
        {
          name: 'de',
          first: 2,
          count: 3,
          children: [leaf('/de', 2), { name: 'produkte', first: 3, count: 2, children: [leaf('a', 3), leaf('b', 4)] }],
        },
      ],
    },
  ];
  return flatten({ pages, tree });
}

const byName = (tree: PageTree, name: string) => {
  const node = tree.nodes.find((candidate) => candidate.name === name);
  if (node === undefined) throw new Error(`no node ${name}`);
  return node;
};

describe('the start page tree', () => {
  it('flattens the core tree with levels, parents and children', () => {
    const tree = sample();
    expect(tree.roots).toEqual([0]);
    expect(tree.nodes.map((node) => node.name)).toEqual(['https://a.test', '/', 'about', 'de', '/de', 'produkte', 'a', 'b']);
    const produkte = byName(tree, 'produkte');
    expect(produkte.level).toBe(3);
    expect(tree.nodes[produkte.parent]?.name).toBe('de');
    expect(isPage(byName(tree, 'a'))).toBe(true);
    expect(isPage(produkte)).toBe(false);
  });

  it('chooses a whole subtree and shows a partial parent', () => {
    const tree = sample();
    const selection = new Selection(tree.pages.length);
    toggle(byName(tree, 'de'), selection, null, countsOf(selection, null));
    const counts = countsOf(selection, null);
    expect(selection.count).toBe(3);
    expect(checkState(byName(tree, 'de'), counts)).toBe(true);
    expect(checkState(byName(tree, 'produkte'), counts)).toBe(true);
    expect(checkState(byName(tree, 'https://a.test'), counts)).toBe('indeterminate');
    expect(checkState(byName(tree, 'about'), counts)).toBe(false);
    expect(selection.urls(tree.pages)).toEqual(['https://a.test/de', 'https://a.test/de/produkte/a', 'https://a.test/de/produkte/b']);
    toggle(byName(tree, 'de'), selection, null, counts);
    expect(selection.count).toBe(0);
  });

  it('a partial node chooses the rest when toggled', () => {
    const tree = sample();
    const selection = new Selection(tree.pages.length);
    toggle(byName(tree, 'a'), selection, null, countsOf(selection, null));
    expect(checkState(byName(tree, 'produkte'), countsOf(selection, null))).toBe('indeterminate');
    toggle(byName(tree, 'produkte'), selection, null, countsOf(selection, null));
    expect(selection.count).toBe(2);
  });

  it('all and none act on the pages a search keeps', () => {
    const tree = sample();
    const scope = search(tree.pages, 'PRODUKTE');
    expect(scope === null ? null : Array.from(scope)).toEqual([0, 0, 0, 1, 1]);
    expect(search(tree.pages, '  ')).toBeNull();
    const selection = new Selection(tree.pages.length);
    selection.setAll(true, scope);
    expect(selection.count).toBe(2);
    const counts = countsOf(selection, scope);
    expect(checkState(byName(tree, 'https://a.test'), counts)).toBe(true);
    expect(nodeCounts(byName(tree, 'de'), counts)).toEqual({ inScope: 2, chosen: 2 });
    selection.setAll(true);
    selection.setAll(false, scope);
    expect(selection.count).toBe(3);
  });

  it('shows the children of open nodes with a page in scope only', () => {
    const tree = sample();
    const selection = new Selection(tree.pages.length);
    const names = (rows: number[]) => rows.map((id) => tree.nodes[id]?.name);
    const rootOnly = visibleRows(tree, (id) => id === 0, countsOf(selection, null));
    expect(names(rootOnly)).toEqual(['https://a.test', '/', 'about', 'de']);
    const everything = visibleRows(tree, () => true, countsOf(selection, null));
    expect(everything).toHaveLength(8);
    const found = visibleRows(tree, () => true, countsOf(selection, search(tree.pages, 'about')));
    expect(names(found)).toEqual(['https://a.test', 'about']);
  });

  it('draws only the rows on screen and a few around them', () => {
    expect(windowOf(0, 280, 28, 10_000, 4)).toEqual({ start: 0, end: 18 });
    expect(windowOf(28 * 5000, 280, 28, 10_000, 4)).toEqual({ start: 4996, end: 5014 });
    expect(windowOf(28 * 9999, 280, 28, 10_000, 4)).toEqual({ start: 9995, end: 10_000 });
  });

  it('stays fluid with 10,000 pages', () => {
    const pages = Array.from({ length: 10_000 }, (_, index) => `https://a.test/s${index % 10}/${index}`);
    const children = Array.from({ length: 10 }, (_, section) => ({
      name: `s${section}`,
      first: section * 1000,
      count: 1000,
      children: Array.from({ length: 1000 }, (_, at) => leaf(String(at), section * 1000 + at)),
    }));
    const tree = flatten({ pages, tree: [{ name: 'https://a.test', first: 0, count: 10_000, children }] });
    const selection = new Selection(pages.length);
    toggle(tree.nodes[0]!, selection, null, countsOf(selection, null));
    const counts = countsOf(selection, null);
    expect(selection.count).toBe(10_000);
    expect(visibleRows(tree, () => true, counts)).toHaveLength(10_011);
    expect(checkState(tree.nodes[0]!, counts)).toBe(true);
  });

  it('names the chosen pages and the views for sitemap.capture', () => {
    expect(captureArgs('1-1', ['https://a.test/'], null, 'all')).toEqual({ discovery: '1-1', urls: ['https://a.test/'] });
    expect(captureArgs('1-1', ['https://a.test/'], 'local', 'visible')).toEqual({
      discovery: '1-1',
      urls: ['https://a.test/'],
      env: 'local',
      views: 'visible',
    });
  });
});
