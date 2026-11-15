import { describe, expect, it } from 'vitest';
import { xmlRoots } from './fixtures';
import { JsonNode } from './nodes';
import { searchNodes } from './search';
import { Tree } from './tree';

const DOC = { a: { b: [1, 2], c: 'x' }, d: [{ e: true }], f: null };

function names(tree: Tree): string[] {
  return tree.nodes.map((node, i) => `${tree.levels[i]}:${node.name ?? '$'}`);
}

describe('the tree rows', () => {
  it('open to a depth', () => {
    const tree = new Tree([new JsonNode(DOC)]);
    tree.expandToDepth(1);
    expect(names(tree)).toEqual(['0:$', '1:a', '1:d', '1:f']);
    tree.expandToDepth(2);
    expect(names(tree)).toEqual(['0:$', '1:a', '2:b', '2:c', '1:d', '2:0', '1:f']);
    tree.expandToDepth(0);
    expect(names(tree)).toEqual(['0:$']);
  });

  it('expand and collapse one row, keeping what was open below', () => {
    const tree = new Tree([new JsonNode(DOC)]);
    tree.expandToDepth(2);
    tree.toggle(2);
    expect(names(tree)).toEqual(['0:$', '1:a', '2:b', '3:0', '3:1', '2:c', '1:d', '2:0', '1:f']);
    tree.toggle(1);
    expect(names(tree)).toEqual(['0:$', '1:a', '1:d', '2:0', '1:f']);
    tree.toggle(1);
    expect(names(tree)).toEqual(['0:$', '1:a', '2:b', '3:0', '3:1', '2:c', '1:d', '2:0', '1:f']);
    expect(tree.isOpen(tree.nodes[2] as JsonNode)).toBe(true);
  });

  it('keep to the row budget, opening the upper levels first', () => {
    const wide = new JsonNode({ list: Array.from({ length: 100 }, (_, i) => ({ i })) });
    const tree = new Tree([wide]);
    expect(tree.expandToDepth(3, 50)).toBe(false);
    expect(tree.length).toBe(2);
    expect(tree.expandToDepth(3, 500)).toBe(true);
    expect(tree.length).toBe(202);
  });

  it('expand everything up to the budget, and collapse everything', () => {
    const tree = new Tree([new JsonNode(DOC)]);
    expect(tree.expandAll()).toBe(true);
    expect(tree.length).toBe(10);
    tree.collapseAll();
    expect(names(tree)).toEqual(['0:$']);
  });

  it('find the parent row', () => {
    const tree = new Tree([new JsonNode(DOC)]);
    tree.expandToDepth(2);
    expect(tree.parentIndex(3)).toBe(1);
    expect(tree.parentIndex(0)).toBe(-1);
  });

  it('holds result rows of their own', () => {
    const root = new JsonNode(DOC);
    const results = new Tree(searchNodes([root], 'x').nodes, true);
    expect(results.labelled).toBe(true);
    expect(results.nodes.map((node) => node.path())).toEqual(['$.a.c']);
  });
});

describe('the text search', () => {
  it('matches names and values without case', () => {
    const root = new JsonNode({ Name: 'Ada', list: ['ada lovelace', 3] });
    expect(searchNodes([root], 'ADA').nodes.map((node) => node.path())).toEqual(['$.Name', '$.list[0]']);
    expect(searchNodes([root], 'name').nodes.map((node) => node.path())).toEqual(['$.Name']);
    expect(searchNodes([root], '3').nodes.map((node) => node.path())).toEqual(['$.list[1]']);
  });

  it('matches XML tags, attributes and inline text on the element', () => {
    const roots = xmlRoots('<feed><entry id="e1"><title>Hello</title></entry><!-- hello --></feed>');
    expect(searchNodes(roots, 'hello').nodes.map((node) => node.path())).toEqual([
      '/feed/entry/title',
      '/feed/comment()',
    ]);
    expect(searchNodes(roots, 'e1').nodes.map((node) => node.path())).toEqual(['/feed/entry']);
  });

  it('stops at the limit and says so', () => {
    const root = new JsonNode(Array.from({ length: 20 }, () => 'x'));
    const found = searchNodes([root], 'x', 5);
    expect(found.nodes.length).toBe(5);
    expect(found.truncated).toBe(true);
  });
});
