// The viewer's elements inside its closed shadow root: the tool bar (view modes, filter, expand,
// collapse, copy), the notice and error lines, the tree, the raw text and the status line with
// the active row's path. Built once; viewer.ts wires them.

import { el } from './dom';
import { t } from './labels';
import type { Theme, ViewMode } from './types';
import { TreeView } from './view';

export interface Ui {
  app: HTMLElement;
  modes: Record<ViewMode, HTMLButtonElement>;
  filter: HTMLInputElement;
  expandAll: HTMLButtonElement;
  collapseAll: HTMLButtonElement;
  copyValue: HTMLButtonElement;
  copyPath: HTMLButtonElement;
  notice: HTMLElement;
  error: HTMLElement;
  raw: HTMLPreElement;
  status: HTMLElement;
  view: TreeView;
}

export const MODES: readonly ViewMode[] = ['tree', 'raw', 'pretty'];

function button(doc: Document, label: string): HTMLButtonElement {
  const node = el(doc, 'button');
  node.type = 'button';
  node.textContent = label;
  return node;
}

export function buildUi(doc: Document, kind: 'json' | 'xml', theme: Theme): Ui {
  const app = el(doc, 'div', 'app');
  app.dataset.theme = theme;
  app.setAttribute('role', 'region');
  app.setAttribute('aria-label', t('viewer.label'));

  const bar = el(doc, 'div', 'bar');
  bar.setAttribute('role', 'toolbar');
  const group = el(doc, 'div', 'group');
  group.setAttribute('role', 'group');
  group.setAttribute('aria-label', t('viewer.modes'));
  const modes = {} as Record<ViewMode, HTMLButtonElement>;
  for (const mode of MODES) {
    modes[mode] = button(doc, t(`viewer.${mode}`));
    modes[mode].dataset.mode = mode;
    group.append(modes[mode]);
  }
  const filter = el(doc, 'input');
  filter.type = 'search';
  filter.spellcheck = false;
  filter.autocomplete = 'off';
  filter.setAttribute('aria-label', t('viewer.filter'));
  filter.placeholder = t(kind === 'json' ? 'viewer.filterJson' : 'viewer.filterXml');
  const expandAll = button(doc, t('viewer.expandAll'));
  const collapseAll = button(doc, t('viewer.collapseAll'));
  const copyValue = button(doc, t('viewer.copyValue'));
  const copyPath = button(doc, t('viewer.copyPath'));
  bar.append(group, filter, expandAll, collapseAll, copyValue, copyPath);

  const notice = el(doc, 'div', 'notice');
  notice.setAttribute('role', 'status');
  const error = el(doc, 'div', 'error');
  error.setAttribute('role', 'alert');

  const body = el(doc, 'div', 'body');
  const view = new TreeView(doc, t('viewer.label'));
  const raw = el(doc, 'pre', 'raw');
  raw.tabIndex = 0;
  raw.hidden = true;
  body.append(view.element, raw);

  const status = el(doc, 'div', 'status');
  app.append(bar, notice, error, body, status);
  return { app, modes, filter, expandAll, collapseAll, copyValue, copyPath, notice, error, raw, status, view };
}
