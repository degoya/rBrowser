import { describe, expect, it } from 'vitest';
import { type Answer, batches, distinct, findingsOf, type Item, items, label, loose, nest, problemsOnly, statusLine, tabBadge, text } from './model';

const strings = {
  'plugin.outline.panel.only': 'only {views}',
  'plugin.outline.panel.hiddenEverywhere': 'hidden in every view',
  'plugin.outline.panel.noText': '(no text)',
  'plugin.outline.panel.altMissing': '(alt missing)',
  'plugin.outline.findings.heading-skip': 'Skips from {from} to {to}',
  'plugin.outline.findings.landmark-main-missing': 'No main',
  'plugin.outline.findings.landmark-differs': '{text} shows in {shown} only',
};

const cell = (path: string, visible: boolean) => ({ index: 0, visible, hidden: !visible, selector: path, path });

const answer: Answer = {
  views: [
    { id: 1, label: 'Desktop' },
    { id: 2, label: 'Mobile' },
  ],
  findings: [
    { rule: 'heading-skip', severity: 'warning', message: 'plugin.outline.findings.heading-skip', params: { from: 'h2', to: 'h4' }, view: 1, path: 'h4' },
    { rule: 'link-empty', severity: 'error', message: 'plugin.outline.findings.link-empty', params: {}, view: 1, path: 'a' },
  ],
  sections: {
    headings: {
      rows: [
        { item: { level: 2, text: 'Intro' }, views: { '1': cell('h2', true), '2': cell('h2', true) } },
        { item: { level: 4, text: 'Details' }, views: { '1': cell('h4', true), '2': cell('h4', true) } },
      ],
    },
    landmarks: {
      rows: [
        { item: { role: 'navigation', name: 'Main', tag: 'nav' }, views: { '1': cell('nav', true), '2': cell('nav', false) }, only: [1] },
        { item: { role: 'navigation', name: 'Mobile', tag: 'nav' }, views: { '1': cell('nav2', false) }, only: [] },
      ],
    },
    axtree: { views: { '2': { nodes: [{ depth: 0, role: 'RootWebArea', name: 'Page', backendNodeId: 5 }] } } },
    custom: {
      selectors: [
        { label: 'CMS', selector: '[data-cms-block]', invalid: false, counts: { '1': 3, '2': 2 }, rows: [{ item: { text: 'Promo' }, views: { '1': cell('div', true) }, only: [1] }] },
      ],
    },
  },
};

describe('the panel model', () => {
  it('fills texts and falls back to the key', () => {
    expect(text(strings, 'panel.only', { views: 'Desktop' })).toBe('only Desktop');
    expect(text(strings, 'panel.unknown')).toBe('panel.unknown');
  });

  it('marks what only some views show and what no view shows', () => {
    const list = items(strings, 'landmarks', answer);
    expect(list[0]?.badge).toBe('only Desktop');
    expect(list[0]?.presence.map((p) => p.state)).toEqual(['visible', 'hidden']);
    expect(list[1]?.badge).toBe('hidden in every view');
    expect(list[1]?.presence.map((p) => p.state)).toEqual(['hidden', 'missing']);
    expect(list[0]?.targets).toEqual([
      { view: 1, selector: 'nav' },
      { view: 2, selector: 'nav' },
    ]);
  });

  it('nests headings by level and puts the finding at its heading', () => {
    const list = items(strings, 'headings', answer);
    expect(list.map((item) => [item.depth, item.label])).toEqual([
      [1, 'H2 Intro'],
      [3, 'H4 Details'],
    ]);
    expect(list[1]?.notes).toEqual([{ text: 'Skips from h2 to h4', severity: 'warning' }]);
    expect(list[0]?.notes).toEqual([]);
    expect(findingsOf(answer, 'headings')).toHaveLength(1);
  });

  it('shows the accessibility tree of the chosen view and marks by node', () => {
    const list = items(strings, 'axtree', answer, 2);
    expect(list[0]?.label).toBe('RootWebArea Page');
    expect(list[0]?.targets).toEqual([{ view: 2, backendNodeId: 5 }]);
    expect(items(strings, 'axtree', answer, 1)).toEqual([]);
  });

  it('groups custom selector hits under their selector with counts per view', () => {
    const list = items(strings, 'custom', answer);
    expect(list[0]?.label).toBe('CMS ([data-cms-block])');
    expect(list[0]?.badge).toBe('Desktop 3, Mobile 2');
    expect(list[1]?.depth).toBe(1);
  });

  it('labels links, images and fields', () => {
    expect(label(strings, 'links', { text: '', href: 'https://x.test/', internal: false, blankUnsafe: true })).toBe(
      '(no text) -> https://x.test/ (panel.external, panel.blankUnsafe)',
    );
    expect(label(strings, 'images', { alt: null, src: 'a.png', width: 1, height: 2, naturalWidth: 3, naturalHeight: 4 })).toBe(
      '(alt missing) a.png (1x2 / 3x4)',
    );
    expect(label(strings, 'forms', { label: 'Email', type: 'email', name: 'email', required: true })).toBe(
      'Email [email, name=email, required]',
    );
  });

  it('batches the http links without fragments at the pace', () => {
    expect(batches(['https://a.test/x#1', 'https://a.test/x#2', 'mailto:a@b', undefined, 'http://b.test/', 'https://c.test/'], 2)).toEqual([
      ['https://a.test/x', 'http://b.test/'],
      ['https://c.test/'],
    ]);
    expect(batches(['https://a.test/'], 99)).toEqual([['https://a.test/']]);
    expect(batches(['https://a.test/', 'https://b.test/'], 0)).toEqual([['https://a.test/'], ['https://b.test/']]);
  });
});

describe('the tree for Nuxt UI', () => {
  const item = (depth: number, label: string): Item => ({ depth, label, notes: [], presence: [], targets: [] });

  it('nests each item under the last shallower one, all open', () => {
    const tree = nest([item(0, 'H1'), item(1, 'H2 a'), item(2, 'H3'), item(1, 'H2 b'), item(0, 'H1 b'), item(2, 'H3 deep')]);
    const shape = (nodes: ReturnType<typeof nest>): unknown => nodes.map((node) => (node.children ? [node.label, shape(node.children)] : node.label));
    expect(shape(tree)).toEqual([
      ['H1', [['H2 a', ['H3']], 'H2 b']],
      ['H1 b', ['H3 deep']],
    ]);
    expect(tree[0]?.defaultExpanded).toBe(true);
    expect(tree.map((node) => node.key)).toEqual(['0', '4']);
  });

  it('keeps a flat list flat', () => {
    expect(nest([item(0, 'a'), item(0, 'b')]).map((node) => node.children)).toEqual([undefined, undefined]);
    expect(nest([])).toEqual([]);
  });
});

describe('the status line', () => {
  it('says every part in one line with the most urgent tone', () => {
    expect(statusLine([null, { tone: 'info', text: '' }])).toBeNull();
    expect(
      statusLine([
        { tone: 'warn', text: 'No outline from mobile (Chromium views only).' },
        { tone: 'ok', text: 'Marked in 3 of 4 views.' },
      ]),
    ).toEqual({ tone: 'warn', text: 'No outline from mobile (Chromium views only). Marked in 3 of 4 views.' });
    expect(statusLine([{ tone: 'info', text: 'a' }, { tone: 'error', text: 'b' }, { tone: 'busy', text: 'c' }])?.tone).toBe('error');
    expect(statusLine([{ tone: 'ok', text: 'a' }, { tone: 'busy', text: 'c' }])?.tone).toBe('busy');
  });
});

describe('the findings in the panel (RB-091-03)', () => {
  const page: Answer = {
    views: [
      { id: 1, label: 'Desktop' },
      { id: 2, label: 'Mobile' },
    ],
    findings: [
      { rule: 'landmark-main-missing', severity: 'warning', message: 'plugin.outline.findings.landmark-main-missing', params: {}, view: 1, path: 'html', selector: 'html' },
      { rule: 'landmark-main-missing', severity: 'warning', message: 'plugin.outline.findings.landmark-main-missing', params: {}, view: 2, path: 'html', selector: 'html' },
      { rule: 'landmark-differs', severity: 'info', message: 'plugin.outline.findings.landmark-differs', params: { text: 'nav', shown: 'Desktop' }, view: 2, path: 'nav', selector: '#nav' },
      { rule: 'content-outside-landmarks', severity: 'info', message: 'plugin.outline.findings.content-outside-landmarks', params: { text: 'Loose' }, view: 1, path: 'p', selector: '#loose' },
    ],
    sections: {
      landmarks: {
        rows: [
          { item: { role: 'navigation', name: 'Main', tag: 'nav' }, views: { '1': cell('nav', true) }, only: [1] },
          { item: { role: 'banner', name: '', tag: 'header' }, views: { '1': cell('header', true), '2': cell('header', true) } },
        ],
      },
    },
  };

  it('puts a finding of a view that lacks the entry at the row of the view that has it', () => {
    const list = items(strings, 'landmarks', page);
    expect(list[0]?.notes).toEqual([{ text: 'nav shows in Desktop only', severity: 'info' }]);
    expect(list[1]?.notes).toEqual([]);
  });

  it('lists the findings without a row once, with every view to mark', () => {
    expect(loose(strings, 'landmarks', page)).toEqual([
      { text: 'No main', severity: 'warning', targets: [{ view: 1, selector: 'html' }, { view: 2, selector: 'html' }] },
      { text: 'plugin.outline.findings.content-outside-landmarks', severity: 'info', targets: [{ view: 1, selector: '#loose' }] },
    ]);
    expect(loose(strings, 'headings', page)).toEqual([]);
  });

  it('keeps only the entries with a finding when asked', () => {
    const list = items(strings, 'landmarks', page);
    expect(problemsOnly(list).map((item) => item.label)).toEqual(['navigation "Main" <nav>']);
  });

  it('counts a finding once however many views report it, coloured by the worst', () => {
    const tab = findingsOf(page, 'landmarks');
    expect(distinct(tab)).toHaveLength(3);
    expect(tabBadge(tab)).toEqual({ label: '3', color: 'warning' });
    expect(tabBadge([])).toBeUndefined();
    expect(tabBadge(findingsOf(answer, 'links'))).toEqual({ label: '1', color: 'error' });
  });
});
