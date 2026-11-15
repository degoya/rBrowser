// A short CSS selector for an element, for reports and logs: an id when it is unique, else the
// element's tag, up to two classes and its position among its siblings of the same tag, joined
// upwards with `>` until the selector matches the element alone.

/** The longest selector a report keeps (src/report.rs `MAX_SELECTOR_CHARS`). */
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

function step(element: Element): string {
  let part = element.localName;
  const classes = Array.from(element.classList).slice(0, 2);
  part += classes.map((name) => `.${escape(name)}`).join('');
  const parent = element.parentElement;
  if (parent) {
    const twins = Array.from(parent.children).filter((child) => child !== element && matches(child, part));
    if (twins.length > 0) {
      const same = Array.from(parent.children).filter((child) => child.localName === element.localName);
      part += `:nth-of-type(${same.indexOf(element) + 1})`;
    }
  }
  return part;
}

/** A selector for `element` within its tree (the document, or the shadow root it is in), at most
 *  {@link MAX_SELECTOR_CHARS} characters. */
export function cssPath(element: Element): string {
  const root = element.getRootNode();
  // A shadow root is a document fragment; anything else is searched from the document.
  const doc: ParentNode = root.nodeType === 11 ? (root as ShadowRoot) : element.ownerDocument;
  const parts: string[] = [];
  let node: Element | null = element;
  while (node && parts.length < MAX_DEPTH) {
    if (node.id && unique(doc, `#${escape(node.id)}`, node)) {
      parts.unshift(`#${escape(node.id)}`);
      break;
    }
    parts.unshift(step(node));
    const selector = parts.join(' > ');
    if (unique(doc, selector, element) || node.localName === 'body' || node.localName === 'html') break;
    node = node.parentElement;
  }
  return parts.join(' > ').slice(0, MAX_SELECTOR_CHARS);
}
