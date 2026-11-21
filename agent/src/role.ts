// The ARIA role and accessible name of an element, enough of both to find a control again in
// another view: the explicit `role`, the implicit role of the common elements, and the name
// from aria-labelledby, aria-label, the labels, alt, value, the content or title/placeholder.
// Not the full accname algorithm; what it does not know falls through to text or CSS path. The
// texts are the originals behind a pseudo-localisation (RB-092-02).

import { originalText } from './locale/originals';

/** Names longer than this are no selector. */
export const MAX_NAME = 120;

/** Roles whose name comes from their content. */
const NAME_FROM_CONTENT = new Set([
  'button', 'link', 'heading', 'checkbox', 'radio', 'switch', 'tab', 'menuitem',
  'menuitemcheckbox', 'menuitemradio', 'option', 'treeitem', 'cell', 'columnheader', 'rowheader',
]);

/** Collapses white space and trims. */
export function normalize(text: string | null | undefined): string {
  return (text ?? '').replace(/\s+/g, ' ').trim();
}

function inputRole(input: HTMLInputElement): string | null {
  switch (input.type) {
    case 'button':
    case 'submit':
    case 'reset':
    case 'image':
      return 'button';
    case 'checkbox':
      return 'checkbox';
    case 'radio':
      return 'radio';
    case 'range':
      return 'slider';
    case 'number':
      return 'spinbutton';
    case 'search':
      return 'searchbox';
    case 'email':
    case 'tel':
    case 'text':
    case 'url':
      return 'textbox';
    default:
      return null;
  }
}

/** The element's role: the first token of `role`, else the implicit one, else null. */
export function roleOf(element: Element): string | null {
  const explicit = normalize(element.getAttribute('role')).split(' ')[0];
  if (explicit) return explicit;
  switch (element.localName) {
    case 'button':
      return 'button';
    case 'a':
    case 'area':
      return element.hasAttribute('href') ? 'link' : null;
    case 'input':
      return inputRole(element as HTMLInputElement);
    case 'textarea':
      return 'textbox';
    case 'select': {
      const select = element as HTMLSelectElement;
      return select.multiple || select.size > 1 ? 'listbox' : 'combobox';
    }
    case 'option':
      return 'option';
    case 'h1':
    case 'h2':
    case 'h3':
    case 'h4':
    case 'h5':
    case 'h6':
      return 'heading';
    case 'img':
      return element.getAttribute('alt') ? 'img' : null;
    case 'nav':
      return 'navigation';
    case 'summary':
      return 'button';
    default:
      return null;
  }
}

function byIds(element: Element, ids: string): string {
  const root = element.getRootNode() as Document | ShadowRoot;
  return normalize(
    ids
      .split(/\s+/)
      .map((id) => (id ? originalText(root.getElementById?.(id)) : ''))
      .join(' '),
  );
}

/** The text of the labels of a form field, without the field's own value. */
function labelText(element: Element): string {
  const labels = (element as HTMLInputElement).labels;
  if (!labels || labels.length === 0) return '';
  return normalize(
    Array.from(labels)
      .map((label) => originalText(label, 'input, select, textarea'))
      .join(' '),
  );
}

/** The element's accessible name, '' when it has none. */
export function nameOf(element: Element, role: string | null = roleOf(element)): string {
  const labelledBy = element.getAttribute('aria-labelledby');
  if (labelledBy) {
    const name = byIds(element, labelledBy);
    if (name) return name;
  }
  const label = normalize(element.getAttribute('aria-label'));
  if (label) return label;
  const tag = element.localName;
  if (tag === 'input' || tag === 'select' || tag === 'textarea') {
    const fromLabels = labelText(element);
    if (fromLabels) return fromLabels;
    const input = element as HTMLInputElement;
    if (tag === 'input' && ['button', 'submit', 'reset'].includes(input.type)) {
      return normalize(input.value);
    }
  }
  if (tag === 'img' || (tag === 'input' && (element as HTMLInputElement).type === 'image')) {
    const alt = normalize(element.getAttribute('alt'));
    if (alt) return alt;
  }
  if (role && NAME_FROM_CONTENT.has(role)) {
    const text = normalize(originalText(element));
    if (text) return text;
  }
  return normalize(element.getAttribute('title') ?? element.getAttribute('placeholder'));
}
