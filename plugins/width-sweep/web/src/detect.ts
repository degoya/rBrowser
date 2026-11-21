// What one width of the sweep finds in the page (RB-093-01). The component evaluates the bundle
// (../src/detect.js) in the plugin's isolated world after each width the host's lease set, and
// reads only:
//
// - overflow: the document scrolls sideways (the root's scrollWidth past its clientWidth); the
//   causes are the outermost elements whose right edge passes the viewport, not clipped by an
//   ancestor and not fixed (as the overlays plugin's overflow measurement, RB-060-03). A `100vw`
//   box beside a classic scrollbar - exactly as wide as the window, overhanging by no more than
//   the scrollbar - is the scrollbar's doing, not the layout's, and no finding; an overflow whose
//   cause is not found names the root;
// - text-clipped: an element with text of its own whose box cuts it off (`overflow` hidden or
//   clip, or `text-overflow: ellipsis`) and whose scrollWidth passes its clientWidth; a box that
//   lets its content scroll, and form fields, are left out;
// - overlap: two visible siblings in the flow (not inline, not absolute or fixed, not
//   transparent, not empty) whose boxes share more than a pixel each way.
//
// It also listens for Escape in the page, kept in the isolated world between the evaluations, so
// the sweep can end on it; `hold.ts` (the command `show`) shares that listener.

import { cssPath, fullPath } from './selector';

export type Kind = 'overflow' | 'text-clipped' | 'overlap';

export interface Issue {
  kind: Kind;
  selector: string;
  path: string;
  other?: string;
}

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The part of `window` the detector reads; a test hands its own. */
export interface SweepWindow {
  innerWidth: number;
  scrollX: number;
  getComputedStyle(element: Element): CSSStyleDeclaration;
  requestAnimationFrame(callback: () => void): number;
  setTimeout(callback: () => void, ms: number): number;
}

export interface ProbeOptions {
  /** The width the lease set: the page is waited for until it has it. */
  width: number;
  kinds: string[];
  /** Forget an Escape pressed before (the sweep's first width). */
  reset?: boolean;
}

export interface ProbeAnswer {
  width: number;
  escape: boolean;
  issues: Issue[];
}

/** The most issues of one kind one width reports. */
export const MAX_PER_KIND = 50;
/** The most elements one kind looks at. */
export const MAX_ELEMENTS = 5000;
/** The most children of one parent compared with each other. */
const MAX_SIBLINGS = 60;
/** Sub-pixel layout: an edge this close to another is on it. */
const TOLERANCE = 0.5;
/** How often, and how many milliseconds apart, the page is asked for the width it was set to. */
const SETTLE_TRIES = 30;
const SETTLE_MS = 16;
/** The longest wait for a frame (a hidden view draws none). */
const FRAME_MS = 50;
const CLIPPING = /^(hidden|clip)$/;
const FIELDS = new Set(['input', 'textarea', 'select', 'button', 'option']);
const STATE = '__rbWidthSweep';

/** Whether two boxes share more than a pixel each way. */
export function overlaps(a: Box, b: Box): boolean {
  const width = Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x);
  const height = Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y);
  return width > 1 && height > 1;
}

function box(element: Element): Box {
  const rect = element.getBoundingClientRect();
  return { x: rect.left, y: rect.top, width: rect.width, height: rect.height };
}

function issue(kind: Kind, element: Element, other?: Element): Issue {
  const found: Issue = { kind, selector: cssPath(element), path: fullPath(element) };
  if (other) found.other = cssPath(other);
  return found;
}

function clips(style: CSSStyleDeclaration): boolean {
  const first = (style.overflow || '').split(' ')[0] ?? '';
  return CLIPPING.test(style.overflowX) || CLIPPING.test(first);
}

function shown(element: Element, style: CSSStyleDeclaration): boolean {
  const rect = element.getBoundingClientRect();
  return rect.width > 0 && rect.height > 0 && style.visibility !== 'hidden' && style.opacity !== '0';
}

/** The elements below `root`, in document order, at most {@link MAX_ELEMENTS}. */
function elements(root: Element): Element[] {
  return Array.from(root.querySelectorAll('*')).slice(0, MAX_ELEMENTS);
}

/** The document's horizontal overflow and its causes. */
export function findOverflow(doc: Document, win: SweepWindow): Issue[] {
  const root = doc.documentElement;
  const client = root.clientWidth;
  const overhang = root.scrollWidth - client;
  if (overhang <= TOLERANCE || !doc.body) return [];
  const scrollbar = Math.max(0, win.innerWidth - client);
  const causes: Element[] = [];
  const visit = (element: Element): void => {
    if (causes.length >= MAX_PER_KIND) return;
    const style = win.getComputedStyle(element);
    if (style.display === 'none' || style.position === 'fixed') return;
    const rect = element.getBoundingClientRect();
    if (rect.width > 0 && rect.right + win.scrollX > client + TOLERANCE) {
      causes.push(element);
      return;
    }
    if (element !== doc.body && clips(style)) return;
    for (const child of Array.from(element.children)) visit(child);
  };
  visit(doc.body);
  const vw100 = (element: Element): boolean => {
    const rect = element.getBoundingClientRect();
    return (
      scrollbar >= 1 &&
      Math.abs(rect.width - win.innerWidth) < TOLERANCE &&
      rect.right + win.scrollX - client <= scrollbar + TOLERANCE
    );
  };
  const real = causes.filter((element) => !vw100(element));
  if (real.length > 0) return real.map((element) => issue('overflow', element));
  if (scrollbar >= 1 && overhang <= scrollbar + TOLERANCE) return [];
  return [issue('overflow', root)];
}

function ownText(element: Element): boolean {
  return Array.from(element.childNodes).some((node) => node.nodeType === 3 && (node.textContent ?? '').trim() !== '');
}

/** Text its box cuts off. */
export function findClipped(doc: Document, win: SweepWindow): Issue[] {
  if (!doc.body) return [];
  const found: Issue[] = [];
  for (const element of elements(doc.body)) {
    if (found.length >= MAX_PER_KIND) break;
    if (FIELDS.has(element.localName) || !ownText(element)) continue;
    const style = win.getComputedStyle(element);
    if (!(clips(style) || style.textOverflow === 'ellipsis') || !shown(element, style)) continue;
    if (element.scrollWidth > element.clientWidth + 1) found.push(issue('text-clipped', element));
  }
  return found;
}

/** Whether `element` is a box in the flow that a sibling should not cover. */
function inFlow(element: Element, win: SweepWindow): boolean {
  const style = win.getComputedStyle(element);
  if (style.display === 'inline' || style.display === 'none' || style.display === 'contents') return false;
  if (style.position === 'absolute' || style.position === 'fixed') return false;
  return shown(element, style);
}

/** Visible siblings in the flow that overlap. */
export function findOverlaps(doc: Document, win: SweepWindow): Issue[] {
  if (!doc.body) return [];
  const found: Issue[] = [];
  const parents = [doc.body, ...elements(doc.body)];
  for (const parent of parents) {
    if (found.length >= MAX_PER_KIND) break;
    if (parent.children.length < 2) continue;
    const children = Array.from(parent.children)
      .slice(0, MAX_SIBLINGS)
      .filter((child) => inFlow(child, win))
      .map((child) => ({ child, box: box(child) }));
    for (let later = 1; later < children.length && found.length < MAX_PER_KIND; later += 1) {
      const current = children[later]!;
      const earlier = children.slice(0, later).find((sibling) => overlaps(sibling.box, current.box));
      if (earlier) found.push(issue('overlap', current.child, earlier.child));
    }
  }
  return found;
}

export interface State {
  escape: boolean;
  listening: Document | null;
}

/** The sweep's state in the isolated world, kept between the evaluations. */
export function state(win: SweepWindow): State {
  const holder = win as unknown as Record<string, State | undefined>;
  let held = holder[STATE];
  if (!held) {
    held = { escape: false, listening: null };
    holder[STATE] = held;
  }
  return held;
}

export function listen(doc: Document, held: State): void {
  if (held.listening === doc) return;
  held.listening = doc;
  doc.addEventListener(
    'keydown',
    (event) => {
      if (event.key === 'Escape') held.escape = true;
    },
    true,
  );
}

/** Waits until the page has `width` (or gives up and takes the one it has), then a frame. */
export async function settle(win: SweepWindow, width: number): Promise<void> {
  for (let tries = 0; tries < SETTLE_TRIES && Math.round(win.innerWidth) !== width; tries += 1) {
    await new Promise<void>((resolve) => win.setTimeout(resolve, SETTLE_MS));
  }
  await new Promise<void>((resolve) => {
    let done = false;
    const finish = (): void => {
      if (!done) {
        done = true;
        resolve();
      }
    };
    win.requestAnimationFrame(finish);
    win.setTimeout(finish, FRAME_MS);
  });
}

/** One width: the width the page has, Escape since the reset, the issues of the kinds asked. */
export async function probe(options: ProbeOptions, doc: Document = document, win: SweepWindow = window): Promise<ProbeAnswer> {
  const held = state(win);
  if (options.reset) held.escape = false;
  listen(doc, held);
  await settle(win, options.width);
  const kinds = new Set(options.kinds);
  const issues: Issue[] = [];
  if (kinds.has('overflow')) issues.push(...findOverflow(doc, win));
  if (kinds.has('text-clipped')) issues.push(...findClipped(doc, win));
  if (kinds.has('overlap')) issues.push(...findOverlaps(doc, win));
  return { width: Math.round(win.innerWidth), escape: held.escape, issues };
}
