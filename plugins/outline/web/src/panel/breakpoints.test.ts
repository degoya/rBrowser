import { describe, expect, it } from 'vitest';
import { type DiffAnswer, diffItems, orderItems, showTab } from './breakpoints';
import { type Answer, findingsOf } from './model';

const strings = {
  'plugin.outline.panel.missingIn': 'missing in {views}',
  'plugin.outline.panel.kind.interactive': 'Interactive elements',
  'plugin.outline.panel.kind.headings': 'Headings',
  'plugin.outline.findings.reading-order': 'The {to} shows before the {from}',
  'plugin.outline.rules.reading-order': 'Reading order against the source order',
};

const cell = (path: string, visible: boolean) => ({ index: 0, visible, hidden: !visible, selector: `#${path}`, path });

const views = [
  { id: 1, label: 'Desktop' },
  { id: 2, label: 'Tablet' },
  { id: 3, label: 'Phone' },
];

const DIFF: DiffAnswer = {
  views,
  findings: [],
  rows: [
    { kind: 'headings', label: 'H2 Offers', views: { '1': cell('h2', true), '2': cell('h2', true), '3': cell('h2', false) }, shown: [1, 2], missing: [3] },
    { kind: 'interactive', label: 'link "Pricing"', views: { '1': cell('pricing', true), '3': cell('pricing', false) }, shown: [1], missing: [2, 3] },
    { kind: 'interactive', label: 'button "Menu"', views: { '3': cell('menu', true) }, shown: [3], missing: [1, 2] },
  ],
};

describe('the Breakpoints tab (RB-094-02)', () => {
  it('lists the elements by kind with a dot per breakpoint and the breakpoints missing them', () => {
    const list = diffItems(strings, DIFF);
    expect(list.map((item) => [item.depth, item.label, item.badge])).toEqual([
      [0, 'Interactive elements', undefined],
      [1, 'link "Pricing"', 'missing in Tablet, Phone'],
      [1, 'button "Menu"', 'missing in Desktop, Tablet'],
      [0, 'Headings', undefined],
      [1, 'H2 Offers', 'missing in Phone'],
    ]);
    expect(list[1]?.presence.map((one) => `${one.view.label}:${one.state}`)).toEqual(['Desktop:visible', 'Tablet:missing', 'Phone:hidden']);
    expect(list[1]?.targets).toEqual([
      { view: 1, selector: '#pricing' },
      { view: 3, selector: '#pricing' },
    ]);
    expect(list[0]?.targets).toEqual([]);
  });

  it('shows nothing without differences', () => {
    expect(diffItems(strings, { views, findings: [], rows: [] })).toEqual([]);
  });

  it('counts the findings over the breakpoints on the tab, and the interactive ones on Focus too', () => {
    const answer: Answer = {
      views,
      findings: [
        { rule: 'interactive-differs', severity: 'info', message: 'm', params: {}, view: 2, path: 'nav > a' },
        { rule: 'heading-differs', severity: 'info', message: 'm', params: {}, view: 3, path: 'h2' },
        { rule: 'reading-order', severity: 'warning', message: 'm', params: {}, view: 3, path: 'b' },
        { rule: 'focus-order-jump', severity: 'warning', message: 'm', params: {}, view: 3, path: 'b' },
      ],
    };
    expect(findingsOf(answer, 'breakpoints').map((f) => f.rule)).toEqual(['interactive-differs', 'heading-differs']);
    expect(findingsOf(answer, 'focus').map((f) => f.rule)).toEqual(['interactive-differs', 'focus-order-jump']);
    expect(findingsOf(answer, 'order').map((f) => f.rule)).toEqual(['reading-order', 'focus-order-jump']);
    expect(findingsOf(answer, 'headings').map((f) => f.rule)).toEqual(['heading-differs']);
  });
});

describe('the Reading order tab (RB-094-02)', () => {
  const ORDER: Answer = {
    views,
    findings: [
      {
        rule: 'reading-order',
        severity: 'warning',
        message: 'plugin.outline.findings.reading-order',
        params: { from: 'button "A"', to: 'button "B"' },
        view: 3,
        path: 'b',
        selector: '#b',
        data: { other: { selector: '#a', path: 'a' } },
      },
    ],
  };

  it('lists each finding under its view, a click marks both elements', () => {
    const list = orderItems(strings, ORDER);
    expect(list.map((item) => [item.depth, item.label])).toEqual([
      [0, 'Phone'],
      [1, 'The button "B" shows before the button "A"'],
    ]);
    expect(list[1]?.notes).toEqual([{ text: 'Reading order against the source order', severity: 'warning' }]);
    expect(list[1]?.targets).toEqual([
      { view: 3, selector: '#b' },
      { view: 3, selector: '#a' },
    ]);
    expect(list[0]?.targets).toEqual([
      { view: 3, selector: '#b' },
      { view: 3, selector: '#a' },
    ]);
  });

  it('shows nothing without findings', () => {
    expect(orderItems(strings, { views, findings: [] })).toEqual([]);
  });
});

describe('a command asking for a tab', () => {
  it('names the tab of a show message, nothing else', () => {
    expect(showTab({ type: 'show', tab: 'order' })).toBe('order');
    expect(showTab({ type: 'show', tab: 'breakpoints' })).toBe('breakpoints');
    expect(showTab({ type: 'show', tab: 'nonsense' })).toBeUndefined();
    expect(showTab({ type: 'other', tab: 'order' })).toBeUndefined();
    expect(showTab(null)).toBeUndefined();
  });
});
