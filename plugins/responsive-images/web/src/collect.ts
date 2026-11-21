// The collector (RB-093-02): reads each `<img>` of one view's document - which file the browser
// took from `srcset`/`<picture>`, its own pixels, the size it is drawn at, `sizes`, `loading`,
// `fetchpriority`, whether it is the Largest Contentful Paint element - and measures it at the
// view's density (analyze.ts). The component runs it through `evaluate` in the plugin's isolated
// world of every view (src/collect-entry.ts, built into ../src/collect.js); it only reads the page.
// The one thing it loads is the image the view already shows, decoded again from the cache
// (`new Image()`) for its own pixels, because `naturalWidth` is divided by the srcset density.

import { cssPath } from '../../../../agent/src/selector';
import { analyze, DEFAULTS, type Options } from './analyze';
import { candidateOf, hasWidthDescriptors } from './srcset';
import type { ChosenSource, FileFrom, ImageFacts, PageImages, Request, Size } from './types';

/** The most images listed per view; the rest are counted. */
export const MAX_IMAGES = 300;
/** The longest URL or attribute kept, in characters. */
export const MAX_TEXT = 1000;
/** How long one probe waits for its decode, in milliseconds. */
export const PROBE_MS = 2000;
/** How long the LCP observer waits for the buffered entries, in milliseconds. */
export const LCP_MS = 100;

/** The page as the collector reads it; the tests give their own (happy-dom has no layout). */
export interface Env {
  dpr(): number;
  viewport(): Size;
  /** The content box in CSS px. */
  rendered(image: HTMLImageElement): Size;
  visible(image: HTMLImageElement): boolean;
  natural(image: HTMLImageElement): Size;
  loaded(image: HTMLImageElement): boolean;
  currentSrc(image: HTMLImageElement): string;
  matches(media: string): boolean;
  /** The file's own pixels, decoded again; null when that fails. */
  probe(url: string): Promise<Size | null>;
  /** The LCP element, and whether the page reported one at all. */
  lcpElement(): Promise<{ known: boolean; element: Element | null }>;
}

function px(value: string): number {
  const number = Number.parseFloat(value);
  return Number.isFinite(number) ? number : 0;
}

function withTimeout<T>(promise: Promise<T>, ms: number, fallback: T): Promise<T> {
  return new Promise((resolve) => {
    const timer = setTimeout(() => resolve(fallback), ms);
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      () => {
        clearTimeout(timer);
        resolve(fallback);
      },
    );
  });
}

export const realEnv: Env = {
  dpr: () => window.devicePixelRatio || 1,
  viewport: () => ({ width: window.innerWidth, height: window.innerHeight }),
  rendered(image) {
    const rect = image.getBoundingClientRect();
    const style = getComputedStyle(image);
    const across = px(style.paddingLeft) + px(style.paddingRight) + px(style.borderLeftWidth) + px(style.borderRightWidth);
    const down = px(style.paddingTop) + px(style.paddingBottom) + px(style.borderTopWidth) + px(style.borderBottomWidth);
    return { width: Math.max(0, rect.width - across), height: Math.max(0, rect.height - down) };
  },
  visible(image) {
    const style = getComputedStyle(image);
    return image.getClientRects().length > 0 && style.visibility !== 'hidden';
  },
  natural: (image) => ({ width: image.naturalWidth, height: image.naturalHeight }),
  loaded: (image) => image.complete && image.naturalWidth > 0,
  currentSrc: (image) => image.currentSrc || image.src,
  matches: (media) => window.matchMedia(media).matches,
  probe(url) {
    const probe = new Image();
    probe.src = url;
    const decoded = probe.decode().then(() => (probe.naturalWidth > 0 ? { width: probe.naturalWidth, height: probe.naturalHeight } : null));
    return withTimeout(decoded, PROBE_MS, null);
  },
  lcpElement() {
    return new Promise((resolve) => {
      let known = false;
      let element: Element | null = null;
      const take = (entries: PerformanceEntry[]): void => {
        const last = entries.at(-1) as (PerformanceEntry & { element?: Element | null }) | undefined;
        if (last) {
          known = true;
          element = last.element ?? null;
        }
      };
      try {
        const observer = new PerformanceObserver((list) => take(list.getEntries()));
        observer.observe({ type: 'largest-contentful-paint', buffered: true });
        setTimeout(() => {
          take(observer.takeRecords());
          observer.disconnect();
          resolve({ known, element });
        }, LCP_MS);
      } catch {
        resolve({ known: false, element: null });
      }
    });
  },
};

function cut(text: string): string {
  return text.length > MAX_TEXT ? text.slice(0, MAX_TEXT) : text;
}

function attr(element: Element, name: string): string {
  return element.getAttribute(name) ?? '';
}

/** The node path Lighthouse writes (`getNodePath`): per step the index among the siblings,
 * blank text nodes skipped, and the node's name; `a` for a shadow root. */
export function nodePath(element: Element): string {
  const steps: string[] = [];
  let node: Node | null = element;
  while (node) {
    const parent: Node | null = node instanceof ShadowRoot ? node.host : node.parentNode;
    if (!parent) break;
    if (node instanceof ShadowRoot) {
      steps.unshift('a', node.nodeName);
    } else {
      let index = 0;
      for (let sibling = node.previousSibling; sibling; sibling = sibling.previousSibling) {
        if (sibling.nodeType === Node.TEXT_NODE && (sibling.nodeValue ?? '').trim() === '') continue;
        index += 1;
      }
      steps.unshift(String(index), node.nodeName);
    }
    node = parent;
  }
  return steps.join(',');
}

function isVector(url: string): boolean {
  if (url.startsWith('data:image/svg+xml')) return true;
  const path = url.split(/[?#]/)[0] ?? '';
  return path.toLowerCase().endsWith('.svg');
}

function sourceOf(element: Element): ChosenSource {
  return { media: attr(element, 'media'), type: attr(element, 'type'), srcset: cut(attr(element, 'srcset')), sizes: attr(element, 'sizes') };
}

/** The `<source>` elements of `image`'s picture, and the one its file came from. */
function pictureOf(image: HTMLImageElement, current: string, env: Env): { picture: boolean; source: ChosenSource | null; withoutSizes: boolean } {
  const parent = image.parentElement;
  if (!parent || parent.localName !== 'picture') return { picture: false, source: null, withoutSizes: false };
  const sources = Array.from(parent.children).filter((child) => child.localName === 'source');
  const base = image.baseURI;
  const holds = (element: Element): boolean => candidateOf(attr(element, 'srcset'), current, base) !== null;
  const applies = (element: Element): boolean => {
    const media = attr(element, 'media').trim();
    return media === '' || env.matches(media);
  };
  const chosen = sources.find((element) => applies(element) && holds(element)) ?? sources.find(holds) ?? null;
  const withoutSizes = sources.some((element) => hasWidthDescriptors(attr(element, 'srcset')) && attr(element, 'sizes').trim() === '');
  return { picture: true, source: chosen ? sourceOf(chosen) : null, withoutSizes };
}

/** The file's own pixels: decoded again, else from the descriptor of the candidate it is. */
async function fileOf(image: HTMLImageElement, current: string, srcset: string, natural: Size, env: Env): Promise<[Size, FileFrom]> {
  const probed = await env.probe(current);
  if (probed) return [probed, 'probe'];
  const candidate = candidateOf(srcset, current, image.baseURI);
  if (candidate?.width !== undefined && natural.width > 0) {
    return [{ width: candidate.width, height: Math.round((natural.height * candidate.width) / natural.width) }, 'descriptor'];
  }
  if (candidate?.density !== undefined) {
    return [{ width: Math.round(natural.width * candidate.density), height: Math.round(natural.height * candidate.density) }, 'descriptor'];
  }
  return [natural, 'natural'];
}

async function factsOf(image: HTMLImageElement, env: Env): Promise<ImageFacts> {
  const current = env.currentSrc(image);
  const picture = pictureOf(image, current, env);
  const natural = env.natural(image);
  const loaded = env.loaded(image);
  const vector = isVector(current);
  const srcset = attr(image, 'srcset');
  let file: Size | null = null;
  let fileFrom: FileFrom | null = null;
  if (loaded && !vector) [file, fileFrom] = await fileOf(image, current, picture.source?.srcset ?? srcset, natural, env);
  return {
    selector: cssPath(image),
    path: nodePath(image),
    src: cut(image.getAttribute('src') ? new URL(attr(image, 'src'), image.baseURI).href : ''),
    currentSrc: cut(current),
    srcset: cut(srcset),
    sizes: attr(image, 'sizes'),
    picture: picture.picture,
    source: picture.source,
    sourceWithoutSizes: picture.withoutSizes,
    loading: attr(image, 'loading').toLowerCase(),
    fetchpriority: attr(image, 'fetchpriority').toLowerCase(),
    rendered: env.rendered(image),
    natural,
    file,
    fileFrom,
    vector,
    loaded,
    visible: env.visible(image),
    lcp: false,
  };
}

/** Which image is the LCP one: the element the page reported, else the largest loaded raster
 * image of the first viewport (an estimate, `lcpKnown` false). */
function markLcp(images: HTMLImageElement[], facts: ImageFacts[], lcp: { known: boolean; element: Element | null }, env: Env): void {
  if (lcp.known) {
    const index = images.findIndex((image) => image === lcp.element);
    if (index >= 0) (facts[index] as ImageFacts).lcp = true;
    return;
  }
  const viewport = env.viewport();
  let best = -1;
  let bestArea = 0;
  facts.forEach((one, index) => {
    if (!one.visible || !one.loaded || one.vector) return;
    const top = (images[index] as HTMLImageElement).getBoundingClientRect().top + window.scrollY;
    if (top >= viewport.height) return;
    const area = one.rendered.width * one.rendered.height;
    if (area > bestArea) {
      best = index;
      bestArea = area;
    }
  });
  if (best >= 0) (facts[best] as ImageFacts).lcp = true;
}

function number(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

/** One view's images, measured at the density the request names (the page's without one). */
export async function collect(request: Request, env: Env = realEnv): Promise<PageImages> {
  const all = Array.from(document.querySelectorAll('img'));
  const images = all.slice(0, MAX_IMAGES);
  const asked = number(request.dpr, 0);
  const options: Options = {
    dpr: asked > 0 ? asked : env.dpr(),
    oversizeFactor: number(request.oversizeFactor, DEFAULTS.oversizeFactor),
    undersizeFactor: number(request.undersizeFactor, DEFAULTS.undersizeFactor),
    checkLcp: typeof request.checkLcp === 'boolean' ? request.checkLcp : DEFAULTS.checkLcp,
  };
  const [facts, lcp] = await Promise.all([Promise.all(images.map((image) => factsOf(image, env))), env.lcpElement()]);
  markLcp(images, facts, lcp, env);
  return {
    url: cut(location.href),
    dpr: options.dpr,
    viewport: env.viewport(),
    images: analyze(facts, options),
    more: all.length - images.length,
    lcpKnown: lcp.known,
  };
}
