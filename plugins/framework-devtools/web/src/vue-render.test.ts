import { beforeEach, describe, expect, it } from 'vitest';
import type { Row, StateRow } from './vue-model';
import { renderStateTable, renderTabs, renderTreeList } from './vue-render';

beforeEach(() => {
  document.body.innerHTML = '<nav id="tabs"></nav><ul id="tree"></ul><div id="state"></div>';
});

const el = (id: string): HTMLElement => {
  const element = document.getElementById(id);
  if (!element) throw new Error(id);
  return element;
};

describe('renderTabs', () => {
  it('draws a button per tab, the active one selected, and hands clicks over', () => {
    const chosen: string[] = [];
    renderTabs(el('tabs'), [{ id: 'components', label: 'Components' }, { id: 'pinia', label: 'Pinia' }], 'pinia', (id) =>
      chosen.push(id),
    );
    const buttons = [...el('tabs').querySelectorAll('button')];
    expect(buttons.map((b) => [b.textContent, b.getAttribute('aria-selected')])).toEqual([
      ['Components', 'false'],
      ['Pinia', 'true'],
    ]);
    buttons[0]?.click();
    expect(chosen).toEqual(['components']);
  });
});

describe('renderTreeList', () => {
  const rows: Row[] = [
    { id: 'a', label: 'App', depth: 0, hasChildren: true, expanded: true, tags: [] },
    { id: 'b', label: 'Card', depth: 1, hasChildren: false, expanded: false, tags: ['fragment', '12:00:00.000'] },
  ];

  it('draws the rows with toggles, tags and the selection, and wires the handlers', () => {
    const seen: string[] = [];
    renderTreeList(el('tree'), rows, {
      selected: 'b',
      emptyText: 'none',
      logType: (id) => (id === 'b' ? 'warning' : 'default'),
      onToggle: (id) => seen.push(`toggle ${id}`),
      onSelect: (id) => seen.push(`select ${id}`),
      onHover: (id) => seen.push(`hover ${id}`),
      onOpen: (id) => seen.push(`open ${id}`),
    });
    const items = [...el('tree').querySelectorAll('li')];
    expect(items.map((li) => li.querySelector('.label')?.textContent)).toEqual(['App', 'Card']);
    expect(items[0]?.getAttribute('aria-expanded')).toBe('true');
    expect(items[0]?.querySelector('.toggle')?.textContent).toBe('▾');
    expect(items[1]?.getAttribute('aria-selected')).toBe('true');
    expect(items[1]?.dataset.log).toBe('warning');
    expect(items[0]?.dataset.log).toBeUndefined();
    expect([...(items[1]?.querySelectorAll('.tag') ?? [])].map((tag) => tag.textContent)).toEqual([
      'fragment',
      '12:00:00.000',
    ]);
    items[0]?.querySelector<HTMLButtonElement>('.toggle')?.click();
    items[1]?.click();
    items[1]?.dispatchEvent(new MouseEvent('mouseenter'));
    items[1]?.dispatchEvent(new MouseEvent('dblclick'));
    expect(seen).toEqual(['toggle a', 'select b', 'hover b', 'open b']);
  });

  it('says what is missing when there are no rows', () => {
    renderTreeList(el('tree'), [], { selected: null, emptyText: 'No routes.', onToggle: () => undefined, onSelect: () => undefined });
    expect(el('tree').textContent).toBe('No routes.');
    expect(el('tree').querySelector('.empty')).not.toBeNull();
  });
});

describe('renderStateTable', () => {
  it('draws a heading per section, named, and a row per entry', () => {
    const entries: StateRow[] = [
      { section: 'current', key: 'path', value: '"/x"' },
      { section: 'current', key: 'name', value: '"x"' },
      { section: 'props', key: 'title', value: '"Hi"' },
    ];
    renderStateTable(el('state'), entries, 'nothing', (section) => (section === 'current' ? 'Current route' : section));
    expect([...el('state').querySelectorAll('h3')].map((h) => h.textContent)).toEqual(['Current route', 'props']);
    expect([...el('state').querySelectorAll('.row')].map((row) => row.textContent)).toEqual(['path"/x"', 'name"x"', 'title"Hi"']);
    renderStateTable(el('state'), [], 'nothing');
    expect(el('state').textContent).toBe('nothing');
  });
});
