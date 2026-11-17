// The tab order (RB-070-12) as Chromium walks it, the HTML "sequential focus navigation order":
// each focus navigation scope - the document, an open shadow root, a slot, the document of a
// same-origin iframe - lists its elements with a positive tabindex first (ascending, ties in tree
// order), then those with tabindex 0 in tree order; a scope's own order stands where its owner
// stands, after the owner itself when that is focusable. A scope owner with a negative tabindex
// takes its whole scope out. Not in the order: the plugin's own `rbrowser-*` hosts, inert and
// disabled elements, elements that are not rendered (display:none, visibility:hidden), a negative
// tabindex, all but one radio button of a group. A cross-origin iframe (or every iframe, with
// iframes off) is one stop.

import { cssPath } from '../../selector';
import { fullPath } from '../scan';

/** One stop of the tab order. */
export interface TabStop {
  element: Element;
  /** The tabindex the order sorted it by. */
  tabindex: number;
  /** The iframes from the top document down to the element's document. */
  frames: HTMLIFrameElement[];
  /** An iframe whose document is not walked: one stop for all of it. */
  iframe?: boolean;
}

export interface OrderOptions {
  /** Walk same-origin iframes (setting `tabOrder.includeIframes`). */
  includeIframes: boolean;
}

/** The most elements one walk looks at, and the most stops it keeps. */
export const MAX_VISITED = 20000;
export const MAX_STOPS = 2000;

const NATIVE = [
  'a[href]',
  'area[href]',
  'button',
  'input:not([type="hidden" i])',
  'select',
  'textarea',
  'iframe',
  'audio[controls]',
  'video[controls]',
  '[contenteditable]:not([contenteditable="false" i])',
].join(', ');
const DISABLEABLE = new Set(['button', 'input', 'select', 'textarea', 'fieldset', 'optgroup', 'option']);

function matches(element: Element, selector: string): boolean {
  try {
    return element.matches(selector);
  } catch {
    return false;
  }
}

/** The value of the tabindex attribute, parsed as HTML parses an integer; none when absent or
 *  not a number. */
export function tabindexAttribute(element: Element): number | undefined {
  const raw = element.getAttribute('tabindex');
  const found = raw === null ? null : /^\s*([+-]?\d+)/.exec(raw);
  return found?.[1] === undefined ? undefined : Number.parseInt(found[1], 10);
}

function disabled(element: Element): boolean {
  if (!DISABLEABLE.has(element.localName)) return false;
  if (matches(element, ':disabled')) return true;
  return (element as HTMLButtonElement).disabled === true || element.closest('fieldset[disabled]') !== null;
}

/** Whether `element` is the summary that opens its details element. */
function opensDetails(element: Element): boolean {
  const parent = element.parentElement;
  return element.localName === 'summary' && parent?.localName === 'details' && parent.querySelector(':scope > summary') === element;
}

/** The tabindex `element` takes part in the order with: its attribute, 0 for a natively focusable
 *  element; none when it cannot take focus at all. Negative: focusable, but not by Tab. */
export function tabIndexOf(element: Element): number | undefined {
  if (disabled(element)) return undefined;
  const attribute = tabindexAttribute(element);
  if (attribute !== undefined) return attribute;
  return matches(element, NATIVE) || opensDetails(element) ? 0 : undefined;
}

/** The element's ancestors across shadow roots, nearest first, within its document. */
export function composedAncestors(element: Element): Element[] {
  const found: Element[] = [];
  let node: Element | null = element;
  while (node) {
    const parent: Element | null = node.assignedSlot ?? node.parentElement ?? ((node.parentNode as ShadowRoot | null)?.host ?? null);
    if (parent) found.push(parent);
    node = parent;
  }
  return found;
}

function inert(element: Element): boolean {
  return element.hasAttribute('inert') || composedAncestors(element).some((ancestor) => ancestor.hasAttribute('inert'));
}

/** Whether the element is rendered: no ancestor with display:none, its own visibility visible,
 *  and - where the browser has it - `checkVisibility` agreeing (closed details, content-visibility). */
export function rendered(element: Element): boolean {
  const win = element.ownerDocument.defaultView;
  if (!win) return false;
  const style = win.getComputedStyle(element);
  if (style.display === 'none' || (style.visibility !== '' && style.visibility !== 'visible')) return false;
  if (composedAncestors(element).some((ancestor) => win.getComputedStyle(ancestor).display === 'none')) return false;
  const check = (element as Element & { checkVisibility?: () => boolean }).checkVisibility;
  return typeof check !== 'function' || check.call(element);
}

/** The open shadow root of `element` that takes part in the order. */
function shadowOf(element: Element): ShadowRoot | null {
  const root = element.shadowRoot;
  return root && root.mode === 'open' ? root : null;
}

/** The document of a same-origin iframe; none for a cross-origin one. */
function frameDocument(frame: HTMLIFrameElement): Document | null {
  try {
    return frame.contentDocument;
  } catch {
    return null;
  }
}

interface Entry {
  element: Element;
  /** The tabindex it sorts by within its scope. */
  order: number;
  /** Whether the element itself is a stop. */
  self: boolean;
  /** The iframe stop for an iframe not walked. */
  iframe?: boolean;
  /** The stops of the scope it owns. */
  scope?: () => TabStop[];
}

interface Walk {
  options: OrderOptions;
  visited: number;
  modal: Element | null;
}

function tabbable(element: Element, walk: Walk): number | undefined {
  const index = tabIndexOf(element);
  if (index === undefined || index < 0) return undefined;
  if (walk.modal && walk.modal !== element && !composedAncestors(element).includes(walk.modal)) return undefined;
  return !inert(element) && rendered(element) ? index : undefined;
}

/** The entries of one scope, in tree order: its elements and the scope owners in it. */
function collect(nodes: Iterable<Element>, walk: Walk, frames: HTMLIFrameElement[], out: Entry[]): void {
  for (const element of nodes) {
    walk.visited += 1;
    if (walk.visited > MAX_VISITED) return;
    if (element.localName.startsWith('rbrowser-')) continue;
    const own = tabbable(element, walk);
    const ownerOrder = tabindexAttribute(element) ?? 0;
    const shadow = shadowOf(element);
    if (shadow) {
      const delegates = shadow.delegatesFocus === true;
      out.push({ element, order: own ?? ownerOrder, self: own !== undefined && !delegates, scope: () => order(shadow.children, walk, frames) });
      continue;
    }
    if (element.localName === 'slot' && element.getRootNode().nodeType === 11) {
      const assigned = (element as HTMLSlotElement).assignedElements?.() ?? [];
      out.push({ element, order: ownerOrder, self: false, scope: () => order(assigned.length ? assigned : element.children, walk, frames) });
      continue;
    }
    if (element.localName === 'iframe') {
      if (ownerOrder < 0 || inert(element) || !rendered(element)) continue;
      const frame = element as HTMLIFrameElement;
      const inner = walk.options.includeIframes ? frameDocument(frame) : null;
      if (inner?.documentElement) {
        out.push({ element, order: ownerOrder, self: false, scope: () => order(inner.documentElement.children, walk, [...frames, frame]) });
      } else {
        out.push({ element, order: ownerOrder, self: true, iframe: true });
      }
      continue;
    }
    if (own !== undefined) out.push({ element, order: own, self: true });
    collect(element.children, walk, frames, out);
  }
}

/** The radio input of `entry`, when it is a stop of a named group. */
function radio(entry: Entry): HTMLInputElement | undefined {
  const input = entry.element as HTMLInputElement;
  return entry.self && input.localName === 'input' && input.type === 'radio' && input.name ? input : undefined;
}

/** One radio button per group (its form and name) of the scope: the checked one, else the first. */
function radios(entries: Entry[]): Entry[] {
  const groups: { form: HTMLFormElement | null; name: string; entry: Entry }[] = [];
  for (const entry of entries) {
    const input = radio(entry);
    if (!input) continue;
    const group = groups.find((each) => each.form === input.form && each.name === input.name);
    if (!group) groups.push({ form: input.form, name: input.name, entry });
    else if (input.checked && !(group.entry.element as HTMLInputElement).checked) group.entry = entry;
  }
  return entries.filter((entry) => !radio(entry) || groups.some((group) => group.entry === entry));
}

/** The ordered stops of the scope of `nodes`. */
function order(nodes: Iterable<Element>, walk: Walk, frames: HTMLIFrameElement[]): TabStop[] {
  const entries: Entry[] = [];
  collect(nodes, walk, frames, entries);
  const kept = radios(entries).filter((entry) => entry.order >= 0);
  const positive = kept.filter((entry) => entry.order > 0).sort((left, right) => left.order - right.order);
  const zero = kept.filter((entry) => entry.order === 0);
  return [...positive, ...zero].flatMap((entry) => {
    const stops: TabStop[] = entry.self ? [{ element: entry.element, tabindex: entry.order, frames, ...(entry.iframe ? { iframe: true } : {}) }] : [];
    return entry.scope ? [...stops, ...entry.scope()] : stops;
  });
}

function openModal(doc: Document): Element | null {
  try {
    return doc.querySelector('dialog:modal');
  } catch {
    return null;
  }
}

/** The tab order of `doc`, at most {@link MAX_STOPS} stops. */
export function tabOrder(doc: Document, options: OrderOptions): TabStop[] {
  const root = doc.documentElement;
  if (!root) return [];
  const walk: Walk = { options, visited: 0, modal: openModal(doc) };
  return order(root.children, walk, []).slice(0, MAX_STOPS);
}

/** `parts` of the frames and shadow roots down to `element`, outermost first. */
function chain(element: Element, frames: readonly HTMLIFrameElement[], part: (element: Element) => string): string {
  const parts: string[] = [];
  let node: Element | null = element;
  while (node) {
    parts.unshift(part(node));
    const root = node.getRootNode();
    node = root.nodeType === 11 ? (root as ShadowRoot).host : null;
  }
  for (const frame of [...frames].reverse()) parts.unshift(chain(frame, [], part));
  return parts.join(' >>> ');
}

/** A short selector of `element` that names its frames and shadow hosts too, joined with ` >>> `. */
export function composedSelector(element: Element, frames: readonly HTMLIFrameElement[] = []): string {
  return chain(element, frames, cssPath).slice(0, 300);
}

/** The full path of `element` through its frames and shadow hosts, the same in every view. */
export function composedPath(element: Element, frames: readonly HTMLIFrameElement[] = []): string {
  const path = chain(element, frames, fullPath);
  return path.length > 300 ? path.slice(path.length - 300) : path;
}
