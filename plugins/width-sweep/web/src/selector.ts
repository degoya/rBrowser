// The selectors a finding carries: a short one for people and the highlight, an id when it is
// unique, else the tag, up to two classes and its place among its siblings of the same tag,
// joined upwards with `>` until it matches the element alone (as the overlays plugin's
// web/src/selector.ts); and the full path from the root (the findings' shared form), which the
// finding's stable id is made of - the same element at every width.

/** The longest selector a finding keeps. */
export const MAX_SELECTOR_CHARS = 300;
const MAX_DEPTH = 6;

function escape(text: string): string {
  if (typeof CSS !== 'undefined' && typeof CSS.escape === 'function') return CSS.escape(text);
  return text.replace(/[^a-zA-Z0-9_-]/g, (char) => `\\${char}`);
}

function unique(root: ParentNode, selector: string, element: Element): boolean {
  try {
    const found = root.querySelectorAll(selector);
    return found.length === 1 && found[0] === element;
  } catch {
    return false;
  }
}

function matches(element: Element, selector: string): boolean {
  try {
    return element.matches(selector);
  } catch {
    return false;
  }
}

function place(element: Element): number {
  const parent = element.parentElement;
  if (!parent) return 1;
  const same = Array.from(parent.children).filter((child) => child.localName === element.localName);
  return same.indexOf(element) + 1;
}

function step(element: Element): string {
  let part = element.localName;
  part += Array.from(element.classList)
    .slice(0, 2)
    .map((name) => `.${escape(name)}`)
    .join('');
  const parent = element.parentElement;
  if (parent && Array.from(parent.children).some((child) => child !== element && matches(child, part))) {
    part += `:nth-of-type(${place(element)})`;
  }
  return part;
}

/** A short selector for `element`, at most {@link MAX_SELECTOR_CHARS} characters. */
export function cssPath(element: Element): string {
  const root = element.getRootNode();
  const scope: ParentNode = root.nodeType === 11 ? (root as ShadowRoot) : element.ownerDocument;
  const parts: string[] = [];
  let node: Element | null = element;
  while (node && parts.length < MAX_DEPTH) {
    if (node.id && unique(scope, `#${escape(node.id)}`, node)) {
      parts.unshift(`#${escape(node.id)}`);
      break;
    }
    parts.unshift(step(node));
    if (unique(scope, parts.join(' > '), element) || node.localName === 'body' || node.localName === 'html') break;
    node = node.parentElement;
  }
  return parts.join(' > ').slice(0, MAX_SELECTOR_CHARS);
}

/** The element's path from the root, the findings' shared form (web-common/src/element-path.ts):
 *  `html > body > main > div:nth-of-type(2)`, the same in every plugin. */
export { elementPath as fullPath } from '../../../web-common/src/element-path';
