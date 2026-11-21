// The structured data panel's model (RB-090-96): the answer of `inspect` as Nuxt UI's tree, the
// findings at their nodes, the way to a node, the texts.
import { describe, expect, it } from 'vitest';
import { ANSWER, source, STRINGS } from './fixtures';
import { type Answer, ancestorsOf, findingText, findingsAt, sortedFindings, statusOf, summaryText, text, toTree, type TreeItem, viewItems } from './model';

const find = (items: TreeItem[], key: string): TreeItem | undefined => {
  for (const item of items) {
    if (item.key === key) return item;
    const inner = find(item.children ?? [], key);
    if (inner) return inner;
  }
  return undefined;
};

describe('the tree', () => {
  const tree = toTree(STRINGS, ANSWER);

  it('has a top-level item per node, labelled by its types', () => {
    expect(tree.map((item) => item.key)).toEqual(['n0', 'n1']);
    expect(tree[0]?.label).toBe('Organization');
    expect(tree[0]?.note).toBe('JSON-LD #org');
  });

  it('shows a scalar property on one line and a nested node as a child', () => {
    const article = find(tree, 'n1');
    expect(article?.children?.map((item) => item.label)).toEqual(['author', 'headlinee: typo', 'publisher', 'wordCount: 120']);
    const author = find(tree, 'n1/author');
    expect(author?.children?.[0]?.key).toBe('n2');
    expect(author?.children?.[0]?.label).toBe('Person');
  });

  it('marks a property and a node with their worst finding and its count', () => {
    expect(find(tree, 'n1/headlinee')?.severity).toBe('error');
    const article = find(tree, 'n1');
    expect(article?.severity).toBe('error');
    expect(article?.count).toBe(2);
    expect(find(tree, 'n0')?.severity).toBeUndefined();
  });

  it('resolves a reference to the node it names', () => {
    const publisher = find(tree, 'n1/publisher');
    expect(publisher?.children?.[0]?.target).toBe('n0');
    expect(publisher?.children?.[0]?.label).toBe(text(STRINGS, 'panel.refTo', { target: 'Organization #org' }));
  });

  it('names a reference that leaves the page and a node without a type', () => {
    const outside: Answer = {
      ...ANSWER,
      items: [{ id: 'n0', types: [], source, pointer: '', props: [{ name: 'about', values: [{ kind: 'ref', atId: 'https://example.test/#x', target: null }] }] }],
    };
    const items = toTree(STRINGS, outside);
    expect(items[0]?.label).toBe(text(STRINGS, 'panel.untyped'));
    expect(find(items, 'n0/about')?.children?.[0]?.label).toBe(text(STRINGS, 'panel.refElsewhere', { id: 'https://example.test/#x' }));
    expect(find(items, 'n0/about')?.children?.[0]?.target).toBeUndefined();
  });
});

describe('a node', () => {
  it('is reached through the keys of its ancestors', () => {
    expect(ancestorsOf(toTree(STRINGS, ANSWER), 'n2')).toEqual(['n1', 'n1/author']);
    expect(ancestorsOf(toTree(STRINGS, ANSWER), 'n0')).toEqual([]);
    expect(ancestorsOf(toTree(STRINGS, ANSWER), 'n9')).toBeNull();
  });

  it('lists the findings at it', () => {
    expect(findingsAt(ANSWER, 'n1').map((finding) => finding.rule)).toEqual(['rich.recommended', 'property.unknown']);
  });
});

describe('the texts', () => {
  it('fill a finding message with its params', () => {
    expect(findingText(STRINGS, ANSWER.findings[1]!)).toBe('Article: schema.org has no property headlinee.');
  });

  it('sort the findings errors first', () => {
    expect(sortedFindings(ANSWER).map((finding) => finding.severity)).toEqual(['error', 'warning', 'warning']);
  });

  it('count the findings by severity', () => {
    expect(summaryText(STRINGS, ANSWER)).toBe('1 error, 2 warnings, 0 notes');
  });

  it('offer the active view first, then every view', () => {
    expect(viewItems(STRINGS, ANSWER)).toEqual([
      { label: 'Active view', value: 'active' },
      { label: 'Desktop', value: 1 },
      { label: 'Phone', value: 2 },
    ]);
  });

  it('put a failure, a cut and a truncated answer into one status line', () => {
    expect(statusOf(STRINGS, { failure: 'boom', loading: false, answer: null })).toEqual({ tone: 'error', text: 'boom' });
    expect(statusOf(STRINGS, { failure: null, loading: true, answer: null })?.tone).toBe('busy');
    const line = statusOf(STRINGS, { failure: null, loading: false, answer: { ...ANSWER, cut: true, truncated: true, richResults: false } });
    expect(line?.tone).toBe('warn');
    expect(line?.text).toContain(text(STRINGS, 'panel.cut'));
    expect(line?.text).toContain(text(STRINGS, 'panel.truncated'));
    expect(line?.text).toContain(text(STRINGS, 'panel.richOff'));
    // RB-094-21: "Rich results are not checked (Settings > Checks > Structured data)" links there.
    expect(line?.setting).toEqual({ key: 'structured-data.richResults', label: text(STRINGS, 'panel.setting_link') });
    const cutOnly = statusOf(STRINGS, { failure: null, loading: false, answer: { ...ANSWER, cut: true } });
    expect(cutOnly?.setting).toBeUndefined();
    expect(statusOf(STRINGS, { failure: null, loading: false, answer: ANSWER })).toBeNull();
  });
});
