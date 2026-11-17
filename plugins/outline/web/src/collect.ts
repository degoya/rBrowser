// The collector (RB-070-03): reads one view's document - headings, landmarks, links, images,
// form fields, the focus order and the custom selectors - with each element's selector, path,
// visibility and box. The component runs it through `evaluate` in the plugin's isolated world of
// every view (src/collect-entry.ts, built into ../src/collect.js); it only reads the page.

import { nameOf, normalize, roleOf } from '../../../../agent/src/role';
import { cssPath } from '../../../../agent/src/selector';
import { elementPath } from './path';
import type {
  CustomHit,
  CustomResult,
  CustomSelector,
  Field,
  Focusable,
  Heading,
  Image,
  Landmark,
  Link,
  Located,
  Outside,
  PageOutline,
  Request,
} from './types';
import { type Env, isHidden, isVisible, realEnv, round } from './visible';

/** The most entries of one list; the component cuts at the same number. */
export const MAX_ENTRIES = 500;
/** The most hits listed per custom selector (all are counted). */
export const MAX_HITS = 100;
/** The most custom selectors taken. */
export const MAX_CUSTOM = 20;
/** The longest text kept, in characters. */
export const MAX_TEXT = 200;
/** The longest URL kept, in characters. */
export const MAX_URL = 2048;
/** The most pieces of content outside every landmark listed. */
export const MAX_OUTSIDE = 50;

export const SECTIONS = ['headings', 'landmarks', 'links', 'images', 'forms', 'focus', 'custom'] as const;

const LANDMARK_TAGS: Record<string, string> = {
  header: 'banner',
  nav: 'navigation',
  main: 'main',
  aside: 'complementary',
  footer: 'contentinfo',
  search: 'search',
};
const LANDMARK_ROLES = new Set([
  'banner', 'navigation', 'main', 'complementary', 'contentinfo', 'region', 'search', 'form',
]);
/** `header` and `footer` are landmarks only outside sectioning content. */
const SECTIONING = 'article, aside, main, nav, section';
const FOCUSABLE =
  'a[href], area[href], button, input, select, textarea, iframe, summary, [tabindex], ' +
  '[contenteditable=""], [contenteditable="true"]';
const FIELDS = 'input, select, textarea';
/** Inputs that are no field a person names. */
const NO_LABEL_TYPES = new Set(['hidden', 'submit', 'reset', 'button', 'image']);
/** An image no alt names and nothing marks decorative. */
const UNNAMED_IMAGE = 'img:not([alt]):not([role="presentation"]):not([role="none"]):not([aria-hidden="true"])';
/** Content without text that still shows something. */
const MEDIA = 'img, svg, video, audio, canvas, iframe, object, embed, input, select, textarea, button';
/** Elements that are no content of the page, and dialogs, which axe leaves out of `region`. */
const NOT_CONTENT = new Set(['script', 'style', 'template', 'noscript', 'link', 'meta', 'dialog']);

function cut(text: string, max = MAX_TEXT): string {
  return text.length > max ? text.slice(0, max) : text;
}

function located(element: Element, env: Env): Located {
  const box = round(env.rect(element));
  const hidden = isHidden(element);
  return {
    selector: cssPath(element),
    path: elementPath(element),
    hidden,
    visible: isVisible(element, hidden, box, env),
    box,
  };
}

function capped<T>(elements: Element[], name: string, truncated: string[], make: (el: Element) => T): T[] {
  if (elements.length > MAX_ENTRIES) truncated.push(name);
  return elements.slice(0, MAX_ENTRIES).map(make);
}

function headings(env: Env, truncated: string[]): Heading[] {
  const found = Array.from(document.querySelectorAll('h1, h2, h3, h4, h5, h6, [role="heading"]'));
  return capped(found, 'headings', truncated, (element) => {
    const tagLevel = /^h([1-6])$/.exec(element.localName)?.[1];
    const ariaLevel = Number.parseInt(element.getAttribute('aria-level') ?? '', 10);
    const level = Number.isInteger(ariaLevel) && ariaLevel >= 1 ? Math.min(ariaLevel, 9) : Number(tagLevel ?? 2);
    const full = normalize(element.textContent);
    return {
      ...located(element, env),
      level,
      text: cut(full),
      name: cut(headingName(element)),
      length: full.length,
      image: element.querySelector(UNNAMED_IMAGE) !== null,
    };
  });
}

/** A heading's accessible name: its label or text, else the alts of the images it holds. */
function headingName(element: Element): string {
  const name = nameOf(element, 'heading');
  if (name) return name;
  const alts = Array.from(element.querySelectorAll('img[alt]'), (image) => normalize(image.getAttribute('alt')));
  return alts.filter(Boolean).join(' ');
}

function landmarkRole(element: Element): string | null {
  const explicit = normalize(element.getAttribute('role')).split(' ')[0] ?? '';
  if (explicit) return LANDMARK_ROLES.has(explicit) ? explicit : null;
  const tag = element.localName;
  if ((tag === 'header' || tag === 'footer') && element.parentElement?.closest(SECTIONING)) return null;
  if (tag === 'section' || tag === 'form') {
    const named = element.hasAttribute('aria-label') || element.hasAttribute('aria-labelledby');
    return named ? (tag === 'section' ? 'region' : 'form') : null;
  }
  return LANDMARK_TAGS[tag] ?? null;
}

function landmarkElements(): Element[] {
  const candidates = document.querySelectorAll('header, nav, main, aside, footer, search, section, form, [role]');
  return Array.from(candidates).filter((element) => landmarkRole(element) !== null);
}

function landmarks(env: Env, truncated: string[]): Landmark[] {
  const found = landmarkElements();
  return capped(found, 'landmarks', truncated, (element) => ({
    ...located(element, env),
    role: landmarkRole(element) ?? '',
    tag: element.localName,
    name: cut(nameOf(element, null)),
  }));
}

/** A link to a place on the same page whose target exists (a skip link). */
function isSkipLink(element: Element): boolean {
  const href = element.localName === 'a' ? element.getAttribute('href') : null;
  if (!href || !href.startsWith('#') || href.length < 2) return false;
  try {
    return document.getElementById(decodeURIComponent(href.slice(1))) !== null;
  } catch {
    return false;
  }
}

function isDialog(element: Element): boolean {
  const role = normalize(element.getAttribute('role'));
  return role === 'dialog' || role === 'alertdialog';
}

/** The visible content outside every landmark, as axe's `region` finds it: each outermost element
 * holding text or media and no landmark, below the elements that hold one; skip links, dialogs
 * and what is hidden left out. In document order, at most MAX_OUTSIDE. */
function outside(env: Env): Outside[] {
  const marks = new Set(landmarkElements());
  const around = new Set<Element>();
  for (const mark of marks) {
    for (let node = mark.parentElement; node && !around.has(node); node = node.parentElement) around.add(node);
  }
  const found: Element[] = [];
  const walk = (parent: Element): void => {
    for (const child of Array.from(parent.children)) {
      if (found.length >= MAX_OUTSIDE) return;
      if (marks.has(child) || NOT_CONTENT.has(child.localName) || isDialog(child) || isHidden(child)) continue;
      if (around.has(child)) walk(child);
      else if (!isSkipLink(child) && (normalize(child.textContent) !== '' || child.matches(MEDIA) || child.querySelector(MEDIA))) {
        found.push(child);
      }
    }
  };
  if (document.body) walk(document.body);
  return found.map((element) => ({ ...located(element, env), text: cut(normalize(element.textContent), 80) }));
}

function links(env: Env, truncated: string[]): Link[] {
  const origin = location.origin;
  const found = Array.from(document.querySelectorAll('a[href], area[href]'));
  return capped(found, 'links', truncated, (element) => {
    const anchor = element as HTMLAnchorElement;
    const href = cut(anchor.href, MAX_URL);
    const rel = normalize(anchor.getAttribute('rel')).toLowerCase().split(' ');
    let internal = false;
    try {
      internal = new URL(href).origin === origin;
    } catch {
      internal = false;
    }
    return {
      ...located(element, env),
      href,
      text: cut(nameOf(element, 'link')),
      internal,
      blankUnsafe: anchor.target === '_blank' && !rel.includes('noopener') && !rel.includes('noreferrer'),
      nofollow: rel.includes('nofollow'),
    };
  });
}

function images(env: Env, truncated: string[]): Image[] {
  const found = Array.from(document.querySelectorAll('img'));
  return capped(found, 'images', truncated, (element) => {
    const image = element as HTMLImageElement;
    const role = normalize(image.getAttribute('role'));
    return {
      ...located(element, env),
      src: cut(image.currentSrc || image.src, MAX_URL),
      alt: image.getAttribute('alt'),
      decorative: role === 'presentation' || role === 'none' || image.getAttribute('aria-hidden') === 'true',
      width: Math.round(image.width || 0),
      height: Math.round(image.height || 0),
      naturalWidth: image.naturalWidth || 0,
      naturalHeight: image.naturalHeight || 0,
      lazy: image.loading === 'lazy' || image.getAttribute('loading') === 'lazy',
    };
  });
}

function labelSource(element: Element, label: string): string {
  if (!label) return 'none';
  if (element.getAttribute('aria-labelledby')) return 'aria-labelledby';
  if (normalize(element.getAttribute('aria-label'))) return 'aria-label';
  if ((element as HTMLInputElement).labels?.length) return 'label';
  return normalize(element.getAttribute('title')) ? 'title' : 'placeholder';
}

function fields(env: Env, truncated: string[]): Field[] {
  const found = Array.from(document.querySelectorAll(FIELDS)).filter(
    (element) => !(element.localName === 'input' && NO_LABEL_TYPES.has((element as HTMLInputElement).type)),
  );
  return capped(found, 'fields', truncated, (element) => {
    const field = element as HTMLInputElement;
    const label = cut(nameOf(element));
    return {
      ...located(element, env),
      tag: element.localName,
      type: element.localName === 'input' ? field.type : element.localName,
      name: cut(field.name ?? ''),
      label,
      labelSource: labelSource(element, label),
      autocomplete: cut(normalize(element.getAttribute('autocomplete'))),
      required: element.hasAttribute('required') || element.getAttribute('aria-required') === 'true',
    };
  });
}

function tabIndexOf(element: Element): number {
  const value = Number.parseInt(element.getAttribute('tabindex') ?? '', 10);
  return Number.isInteger(value) ? value : 0;
}

/** Fixed or sticky on the screen, itself or an ancestor. */
function isFixed(element: Element): boolean {
  for (let node: Element | null = element; node; node = node.parentElement) {
    const position = getComputedStyle(node).position;
    if (position === 'fixed' || position === 'sticky') return true;
  }
  return false;
}

function isFocusable(element: Element): boolean {
  if ((element as HTMLButtonElement).disabled) return false;
  if (element.localName === 'input' && (element as HTMLInputElement).type === 'hidden') return false;
  return tabIndexOf(element) >= 0;
}

/** The sequential focus order: positive tabindex first (ascending, stable), then document
 * order; hidden elements are skipped, as the browser does. */
function focus(env: Env, truncated: string[]): Focusable[] {
  const candidates = Array.from(document.querySelectorAll(FOCUSABLE)).filter(isFocusable);
  const ordered = candidates
    .map((element, index) => ({ element, index, tab: tabIndexOf(element) }))
    .sort((a, b) => {
      const ka = a.tab > 0 ? a.tab : Number.MAX_SAFE_INTEGER;
      const kb = b.tab > 0 ? b.tab : Number.MAX_SAFE_INTEGER;
      return ka - kb || a.index - b.index;
    })
    .map(({ element }) => element)
    .filter((element) => !isHidden(element));
  return capped(ordered, 'focus', truncated, (element) => {
    const role = roleOf(element) ?? '';
    return {
      ...located(element, env),
      tag: element.localName,
      role,
      name: cut(nameOf(element, role || null)),
      tabindex: tabIndexOf(element),
      fixed: isFixed(element),
    };
  });
}

function custom(selectors: CustomSelector[], env: Env): CustomResult[] {
  return selectors.slice(0, MAX_CUSTOM).map(({ label, selector }) => {
    let found: Element[];
    try {
      found = Array.from(document.querySelectorAll(selector));
    } catch {
      return { label, selector, invalid: true, count: 0, hits: [] };
    }
    const hits: CustomHit[] = found
      .slice(0, MAX_HITS)
      .map((element) => ({ ...located(element, env), text: cut(normalize(element.textContent), 80) }));
    return { label, selector, invalid: false, count: found.length, hits };
  });
}

/** One view's outline: the sections `request` names (every one without), each list in document
 * order (the focus list in focus order). */
export function collect(request: Request = {}, env: Env = realEnv): PageOutline {
  const wanted = new Set(request.sections?.length ? request.sections : SECTIONS);
  const truncated: string[] = [];
  const pick = <T>(name: string, read: () => T[]): T[] => (wanted.has(name) ? read() : []);
  return {
    url: cut(location.href, MAX_URL),
    lang: cut(normalize(document.documentElement.getAttribute('lang')), 35),
    headings: pick('headings', () => headings(env, truncated)),
    landmarks: pick('landmarks', () => landmarks(env, truncated)),
    outside: pick('landmarks', () => outside(env)),
    links: pick('links', () => links(env, truncated)),
    images: pick('images', () => images(env, truncated)),
    fields: pick('forms', () => fields(env, truncated)),
    focus: pick('focus', () => focus(env, truncated)),
    custom: pick('custom', () => custom(request.custom ?? [], env)),
    truncated,
  };
}
