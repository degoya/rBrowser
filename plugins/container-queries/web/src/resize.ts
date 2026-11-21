// A container's width set by rBrowser - a drag on its overlay handle or `containers_resize` - to
// test its @container rules without changing the view. The width is pinned inline (`width`,
// `min-width` and `max-width`, `!important`, through the CSSOM, which no page CSP refuses), and
// the inline declarations it replaced are kept on the element itself, so the page script and the
// analysis the component evaluates - two scripts that share the DOM and nothing else - see the same
// state, and a reset from either side brings the page's own inline style back.

/** The attribute that holds what a reset restores; its presence marks a resized container. */
export const ORIGINAL_ATTRIBUTE = 'data-rbrowser-container-width';
/** The widest width rBrowser sets, in px. */
export const MAX_WIDTH_PX = 100_000;

const PROPERTIES = ['width', 'min-width', 'max-width'] as const;

interface Original {
  /** Whether the element had a style attribute. */
  had: boolean;
  /** Each property's inline value and priority before the first resize. */
  props: Array<[string, string, string]>;
}

function style(element: Element): CSSStyleDeclaration | null {
  const value = (element as HTMLElement).style;
  return value && typeof value.setProperty === 'function' ? value : null;
}

/** Whether rBrowser set the width of `element`. */
export function isResized(element: Element): boolean {
  return element.hasAttribute(ORIGINAL_ATTRIBUTE);
}

/**
 * Pins the content box of `element` to `px` wide (rounded); the first resize keeps the inline
 * style it replaces.
 *
 * @throws a width that is no finite number from 0 to {@link MAX_WIDTH_PX}.
 */
export function resizeElement(element: Element, px: number): void {
  if (!Number.isFinite(px) || px < 0 || px > MAX_WIDTH_PX) throw new RangeError('width');
  const declarations = style(element);
  if (!declarations) return;
  if (!isResized(element)) {
    const original: Original = {
      had: element.hasAttribute('style'),
      props: PROPERTIES.map((name) => [name, declarations.getPropertyValue(name), declarations.getPropertyPriority(name)]),
    };
    element.setAttribute(ORIGINAL_ATTRIBUTE, JSON.stringify(original));
  }
  const width = `${Math.round(px + borderBoxExtra(element))}px`;
  for (const name of PROPERTIES) declarations.setProperty(name, width, 'important');
}

/**
 * What `width` takes on top of the content box: the horizontal padding and border under
 * `box-sizing: border-box` (the size a container queries, and the analysis reports, is its content
 * box), else nothing. `box-sizing` itself is left alone, so a reset restores exactly what it was.
 */
function borderBoxExtra(element: Element): number {
  const computed = element.ownerDocument.defaultView?.getComputedStyle(element);
  if (!computed || computed.boxSizing !== 'border-box') return 0;
  const px = (name: string): number => parseFloat(computed.getPropertyValue(name)) || 0;
  return px('padding-left') + px('padding-right') + px('border-left-width') + px('border-right-width');
}

function readOriginal(text: string | null): Original | null {
  try {
    const value = JSON.parse(text ?? '') as Partial<Original>;
    if (typeof value.had !== 'boolean' || !Array.isArray(value.props)) return null;
    const props = value.props.filter(
      (entry): entry is [string, string, string] =>
        Array.isArray(entry) && entry.length === 3 && entry.every((part) => typeof part === 'string') && (PROPERTIES as readonly string[]).includes(entry[0]),
    );
    return { had: value.had, props };
  } catch {
    return null;
  }
}

/** Brings back the inline style `element` had before rBrowser resized it; false when it was not. */
export function resetElement(element: Element): boolean {
  if (!isResized(element)) return false;
  const original = readOriginal(element.getAttribute(ORIGINAL_ATTRIBUTE));
  element.removeAttribute(ORIGINAL_ATTRIBUTE);
  const declarations = style(element);
  if (!original || !declarations) return true;
  for (const [name, value, priority] of original.props) {
    if (value === '') declarations.removeProperty(name);
    else declarations.setProperty(name, value, priority);
  }
  if (!original.had && declarations.length === 0) element.removeAttribute('style');
  return true;
}

/** Resets every container of `doc` rBrowser resized; answers how many. */
export function resetAll(doc: Document): number {
  let count = 0;
  for (const element of Array.from(doc.querySelectorAll(`[${ORIGINAL_ATTRIBUTE}]`))) {
    if (resetElement(element)) count += 1;
  }
  return count;
}
