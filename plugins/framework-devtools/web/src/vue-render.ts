// What the Vue panel draws (RB-080-24, RB-080-31): the tab strip, the rows of the tree pane
// (components, a custom inspector's nodes, routes or timeline events) and the sections and rows
// of the state pane. Plain DOM, replaced on every render; the controller (`vue-controller.ts`)
// decides what the rows are.

import type { Row, StateRow } from './vue-model';

/** A tab of the panel. */
export interface Tab {
  id: string;
  label: string;
}

/** What the tree pane's rows do. */
export interface TreeHandlers {
  selected: string | null;
  /** Shown instead of rows when there are none. */
  emptyText: string;
  /** The row's log type (`warning`, `error`) for the timeline's colours, when it has one. */
  logType?: (id: string) => string | undefined;
  onToggle(id: string): void;
  onSelect(id: string): void;
  onHover?: (id: string) => void;
  onOpen?: (id: string) => void;
}

/** The tab strip: a button per tab, the active one selected. */
export function renderTabs(nav: HTMLElement, tabs: readonly Tab[], active: string, onSelect: (id: string) => void): void {
  nav.replaceChildren();
  for (const tab of tabs) {
    const button = document.createElement('button');
    button.type = 'button';
    button.setAttribute('role', 'tab');
    button.setAttribute('aria-selected', String(tab.id === active));
    button.dataset.tab = tab.id;
    button.textContent = tab.label;
    button.addEventListener('click', () => onSelect(tab.id));
    nav.append(button);
  }
}

/** The tree pane's rows: a toggle for a node with children, the label, the tags. */
export function renderTreeList(list: HTMLElement, shown: readonly Row[], handlers: TreeHandlers): void {
  list.replaceChildren();
  if (shown.length === 0) {
    const empty = document.createElement('li');
    empty.className = 'empty';
    empty.textContent = handlers.emptyText;
    list.append(empty);
    return;
  }
  for (const row of shown) {
    const item = document.createElement('li');
    item.setAttribute('role', 'treeitem');
    item.dataset.id = row.id;
    item.style.paddingLeft = `${row.depth * 14 + 4}px`;
    if (row.hasChildren) item.setAttribute('aria-expanded', String(row.expanded));
    if (row.id === handlers.selected) item.setAttribute('aria-selected', 'true');
    const log = handlers.logType?.(row.id);
    if (log !== undefined && log !== 'default') item.dataset.log = log;
    const toggle = document.createElement('button');
    toggle.type = 'button';
    toggle.className = 'toggle';
    toggle.textContent = row.hasChildren ? (row.expanded ? '▾' : '▸') : '';
    toggle.tabIndex = -1;
    toggle.addEventListener('click', (event) => {
      event.stopPropagation();
      handlers.onToggle(row.id);
    });
    const label = document.createElement('span');
    label.className = 'label';
    label.textContent = row.label;
    item.append(toggle, label);
    for (const tag of row.tags) {
      const badge = document.createElement('span');
      badge.className = 'tag';
      badge.textContent = tag;
      item.append(badge);
    }
    item.addEventListener('click', () => handlers.onSelect(row.id));
    if (handlers.onHover) {
      const hover = handlers.onHover;
      item.addEventListener('mouseenter', () => hover(row.id));
    }
    if (handlers.onOpen) {
      const open = handlers.onOpen;
      item.addEventListener('dblclick', () => open(row.id));
    }
    list.append(item);
  }
}

/** The state pane: a heading per section (named by `sectionText`), a row per entry. */
export function renderStateTable(
  table: HTMLElement,
  entries: readonly StateRow[],
  emptyText: string,
  sectionText: (section: string) => string = (section) => section,
): void {
  table.replaceChildren();
  if (entries.length === 0) {
    const line = document.createElement('p');
    line.className = 'empty';
    line.textContent = emptyText;
    table.append(line);
    return;
  }
  let section = '';
  for (const entry of entries) {
    if (entry.section !== section) {
      section = entry.section;
      const heading = document.createElement('h3');
      heading.textContent = sectionText(section);
      table.append(heading);
    }
    const row = document.createElement('div');
    row.className = 'row';
    const key = document.createElement('span');
    key.className = 'key';
    key.textContent = entry.key;
    const value = document.createElement('span');
    value.className = 'value';
    value.textContent = entry.value;
    row.append(key, value);
    table.append(row);
  }
}
