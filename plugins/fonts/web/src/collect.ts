// The collector of the Fonts plugin (RB-070-13): one view's fonts - the `@font-face` rules and
// `document.fonts`, the font files the page loaded (Resource Timing), the text set in each family
// stack, weight and style with samples for the rendered-font check, the size range of each text
// role and the elements the readability rules may report. The component runs it through
// `evaluate` in the plugin's isolated world (src/collect-entry.ts, built into ../src/page.js); it
// only reads the page.

import { cssPath } from '../../../../agent/src/selector';
import { cut, type FaceLike, firstFamily, formatOf, MAX_URL, readFaces, weightOf } from './faces';
import { elementPath } from '../../../web-common/src/element-path';
import type { Box, Candidate, Located, PageFonts, Request, Resource, Role, Sample, Usage } from './types';

export { type FaceLike, firstFamily, parseSrc, weightOf } from './faces';

/** The most text elements read. */
export const MAX_ELEMENTS = 5000;
/** The most usages (stack, weight, style) kept. */
export const MAX_USAGES = 200;
/** The most samples per usage. */
export const MAX_SAMPLES = 3;
/** The most elements per readability list. */
export const MAX_CANDIDATES = 50;
/** The most font files kept. */
export const MAX_RESOURCES = 200;
/** The longest text sample kept, in characters. */
const MAX_TEXT = 80;
/** The most distinct characters beyond ASCII kept per sample. */
const MAX_NON_ASCII = 16;
/** Paragraph-like text of at least this many characters is held to the line height. */
const PARAGRAPH_CHARS = 80;
/** What `line-height: normal` is taken as. */
const NORMAL_LINE_HEIGHT = 1.2;
/** Font hosts whose file URLs carry no extension. */
const FONT_HOSTS = ['fonts.gstatic.com', 'use.typekit.net', 'p.typekit.net'];

/** A Resource Timing entry, as the collector reads it. */
export interface ResourceLike {
  name: string;
  initiatorType: string;
  encodedBodySize: number;
  transferSize: number;
  decodedBodySize: number;
}

/** The page as the collector reads it; the tests give their own (happy-dom has no layout, no
 * `document.fonts`, no Resource Timing). */
export interface Env {
  /** The element's box in CSS px of the viewport. */
  rect(element: Element): Box;
  viewport(): { width: number; height: number };
  scrollY(): number;
  /** Rendered at all: a box, not `visibility: hidden`. */
  shown(element: Element): boolean;
  fontFaces(): FaceLike[];
  resources(): ResourceLike[];
}

export const realEnv: Env = {
  rect(element) {
    const rect = element.getBoundingClientRect();
    return [rect.x, rect.y, rect.width, rect.height];
  },
  viewport: () => ({ width: window.innerWidth, height: window.innerHeight }),
  scrollY: () => window.scrollY,
  shown(element) {
    return element.getClientRects().length > 0 && getComputedStyle(element).visibility === 'visible';
  },
  fontFaces() {
    const set = (document as Document & { fonts?: Iterable<FontFace> }).fonts;
    if (!set) return [];
    return Array.from(set, (face) => ({
      family: face.family,
      weight: face.weight,
      style: face.style,
      display: face.display,
      unicodeRange: face.unicodeRange,
      status: face.status,
    }));
  },
  resources() {
    return typeof performance?.getEntriesByType === 'function'
      ? (performance.getEntriesByType('resource') as unknown as ResourceLike[])
      : [];
  },
};

const SKIPPED = new Set(['script', 'style', 'noscript', 'template', 'head', 'title', 'svg', 'math', 'iframe']);
const INPUT_SKIPPED = new Set(['hidden', 'checkbox', 'radio', 'range', 'color', 'file', 'image', 'submit', 'reset', 'button']);

/** Line height divided by font size: `normal` is 1.2, a number is itself, px and % divided. */
export function lineHeightRatio(value: string, size: number): number {
  const text = value.trim();
  if (text === '' || text === 'normal' || size <= 0) return NORMAL_LINE_HEIGHT;
  const number = Number.parseFloat(text);
  if (!Number.isFinite(number)) return NORMAL_LINE_HEIGHT;
  const ratio = text.endsWith('px') ? number / size : text.endsWith('%') ? number / 100 : number;
  return Math.round(ratio * 100) / 100;
}

function px(value: string): number {
  const number = Number.parseFloat(value);
  return Number.isFinite(number) ? Math.round(number * 100) / 100 : 0;
}

function isFontUrl(url: string): boolean {
  if (formatOf(url)) return true;
  try {
    return FONT_HOSTS.includes(new URL(url).hostname);
  } catch {
    return false;
  }
}

/** The element's own text: its text node children, without the descendants' text. */
function ownText(element: Element): string {
  let text = '';
  for (const node of Array.from(element.childNodes)) {
    if (node.nodeType === Node.TEXT_NODE) text += node.textContent ?? '';
  }
  return text.replace(/\s+/g, ' ').trim();
}

function nonAscii(text: string): string {
  const seen: string[] = [];
  for (const char of text) {
    if (char.charCodeAt(0) > 0x7f && char !== ' ' && !seen.includes(char)) seen.push(char);
    if (seen.length >= MAX_NON_ASCII) break;
  }
  return seen.join('');
}

function roleOf(element: Element): string {
  const tag = element.localName;
  if (/^h[1-6]$/.test(tag)) return tag;
  if (tag === 'input' || tag === 'textarea' || tag === 'select') return 'input';
  return ['p', 'li', 'a', 'button', 'label'].includes(tag) ? tag : 'text';
}

function located(element: Element): Located {
  return { selector: cut(cssPath(element), MAX_URL), path: cut(elementPath(element), MAX_URL) };
}

/** `autocomplete` tokens whose fields hold a secret even as a placeholder's neighbour: the
 * field's text is never read (RB-070-13, Charge 8 review M2). */
const SENSITIVE_AUTOCOMPLETE = /(^|\s)(current-password|new-password|one-time-code|cc-[a-z-]+)(\s|$)/i;

/** The text a field stands for in the report: its placeholder, never what was typed or chosen
 * (a value can be a password, a card number or a one-time code), and nothing for a password
 * field, a field whose `autocomplete` names a secret, or a `select`. */
export function fieldText(element: Element): string {
  if (element.localName === 'select') return '';
  const field = element as HTMLInputElement | HTMLTextAreaElement;
  if (element.localName === 'input' && ((field as HTMLInputElement).type || '').toLowerCase() === 'password') return '';
  if (SENSITIVE_AUTOCOMPLETE.test(element.getAttribute('autocomplete') ?? '')) return '';
  return (element.getAttribute('placeholder') ?? '').replace(/\s+/g, ' ').trim();
}

function isTextField(element: Element): boolean {
  if (element.localName === 'textarea' || element.localName === 'select') return true;
  return element.localName === 'input' && !INPUT_SKIPPED.has(((element as HTMLInputElement).type || 'text').toLowerCase());
}

/** The elements whose own text the page renders, in document order, without the plugins' own
 * `rbrowser-*` elements. */
function textElements(limit: number): { elements: Element[]; cut: boolean } {
  const elements: Element[] = [];
  const root = document.body ?? document.documentElement;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT, {
    acceptNode(node) {
      const name = (node as Element).localName;
      if (name.startsWith('rbrowser-') || SKIPPED.has(name)) return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    },
  });
  for (let node = walker.currentNode as Element | null; node; node = walker.nextNode() as Element | null) {
    if (isTextField(node) || ownText(node)) {
      if (elements.length >= limit) return { elements, cut: true };
      elements.push(node);
    }
  }
  return { elements, cut: false };
}

function number(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function resourcesOf(env: Env): Resource[] {
  return env
    .resources()
    .filter((entry) => typeof entry.name === 'string' && isFontUrl(entry.name))
    .slice(0, MAX_RESOURCES)
    .map((entry) => ({
      url: cut(entry.name, MAX_URL),
      size: number(entry.encodedBodySize, 0) || number(entry.transferSize, 0),
      decoded: number(entry.decodedBodySize, 0),
    }));
}

function preloadsOf(): string[] {
  return Array.from(document.querySelectorAll('link[rel~="preload" i][as="font" i]'))
    .map((link) => (link as HTMLLinkElement).href)
    .filter(Boolean)
    .slice(0, 50)
    .map((href) => cut(href, MAX_URL));
}

/** One view's fonts. `extraUnreadable` names stylesheets known to be unreadable (tests). */
export function collect(request: Request, env: Env = realEnv, extraUnreadable: string[] = []): PageFonts {
  const minText = number(request.minTextPx, 12);
  const minInput = number(request.minInputPx, 16);
  const minLine = number(request.minLineHeight, 1.2);
  const viewport = env.viewport();
  const { faces, unreadable } = readFaces(env.fontFaces(), extraUnreadable);
  const usages = new Map<string, Usage>();
  const roles = new Map<string, Role>();
  const smallText: Candidate[] = [];
  const smallInputs: Candidate[] = [];
  const tightLines: Candidate[] = [];
  const walk = textElements(MAX_ELEMENTS);
  let truncated = walk.cut;
  let read = 0;
  const push = (list: Candidate[], candidate: () => Candidate) => {
    if (list.length < MAX_CANDIDATES) list.push(candidate());
    else truncated = true;
  };
  for (const element of walk.elements) {
    if (!env.shown(element)) continue;
    read += 1;
    const style = getComputedStyle(element);
    const size = px(style.fontSize);
    const role = roleOf(element);
    const text = role === 'input' ? fieldText(element) : ownText(element);
    const ratio = lineHeightRatio(style.lineHeight, size);
    const stack = style.fontFamily.replace(/\s+/g, ' ').trim();
    const weight = weightOf(style.fontWeight);
    const fontStyle = (style.fontStyle || 'normal').toLowerCase();
    const box = env.rect(element);
    const aboveFold = box[1] + env.scrollY() < viewport.height;
    const candidate = (): Candidate => ({ ...located(element), tag: element.localName, size, lineHeight: ratio, text: cut(text, MAX_TEXT) });
    if (role === 'input') {
      if (size < minInput) push(smallInputs, candidate);
    } else if (size < minText) {
      push(smallText, candidate);
    }
    if (['p', 'li', 'text'].includes(role) && text.length >= PARAGRAPH_CHARS && ratio < minLine) push(tightLines, candidate);
    const key = `${stack}|${weight}|${fontStyle}`;
    let usage = usages.get(key);
    if (!usage) {
      if (usages.size >= MAX_USAGES) {
        truncated = true;
        continue;
      }
      usage = { stack: cut(stack, 400), family: cut(firstFamily(stack), 120), weight, style: cut(fontStyle, 40), count: 0, aboveFold: false, sizes: [], samples: [] };
      usages.set(key, usage);
    }
    usage.count += 1;
    if (!usage.sizes.includes(size) && usage.sizes.length < 16) usage.sizes.push(size);
    const extra = nonAscii(text);
    const wanted = usage.samples.length === 0 || (extra && !usage.samples.some((s) => s.nonAscii)) || (aboveFold && !usage.aboveFold);
    if (wanted && usage.samples.length < MAX_SAMPLES) {
      const sample: Sample = { ...located(element), text: cut(text, MAX_TEXT), nonAscii: extra };
      usage.samples.push(sample);
    }
    usage.aboveFold ||= aboveFold;
    const known = roles.get(role);
    if (known) {
      known.count += 1;
      known.min = Math.min(known.min, size);
      known.max = Math.max(known.max, size);
    } else {
      roles.set(role, { role, count: 1, min: size, max: size, first: located(element) });
    }
  }
  return {
    url: cut(location.href, MAX_URL),
    viewport,
    faces,
    sheets: unreadable,
    preloads: preloadsOf(),
    resources: resourcesOf(env),
    usages: [...usages.values()],
    roles: [...roles.values()],
    smallText,
    smallInputs,
    tightLines,
    elements: read,
    truncated,
  };
}
