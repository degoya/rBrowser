// What a device failed to load (RB-090-72). A page shown through the portal can ask for an
// image, a style sheet, a script or data from an origin the device cannot reach - a dev
// server on the machine's loopback that the environment's `assetOrigins` do not list - by a URL
// the portal never read (built by script, inside JSON, a cached answer). The device's own
// failures are the evidence: an element's `error` (capture phase, the event does not bubble),
// a resource timing entry of a loopback origin or with an error status, a `fetch` or an
// `XMLHttpRequest` that fails, and the images already broken when the agent starts. Only the
// origin leaves the page - never a path or a query - each once, at most `MAX_MISSED_ORIGINS`,
// with how the page asked for it (`via`: the element or the resource timing's initiator type).

/** The most distinct origins one page reports. */
export const MAX_MISSED_ORIGINS = 16;

/** The hosts that name the device itself: never the machine that runs rBrowser. */
const LOOPBACK_HOSTS = ['localhost', '127.0.0.1', '[::1]'];
/** The elements whose `error` event names a resource that did not load. */
const LOADING_TAGS = ['IMG', 'SCRIPT', 'LINK', 'SOURCE', 'VIDEO', 'AUDIO'];
/** What `via` may be: one short lower-case word. */
const VIA_WORD = /^[a-z]{1,24}$/;
/** The elements scanned when the agent starts. */
const SCANNED = 'img, script[src], link[href], source, video, audio';

/** The origin of `url` against `base`, `http` and `https` only; `null` for anything else. */
export function originOf(url: string, base: string): string | null {
  try {
    const parsed = new URL(url, base);
    return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? parsed.origin : null;
  } catch {
    return null;
  }
}

/** Whether `origin` is the device's own loopback, which a page of the portal never reaches. */
export function isLoopbackOrigin(origin: string): boolean {
  try {
    return LOOPBACK_HOSTS.includes(new URL(origin).hostname);
  } catch {
    return false;
  }
}

/** The URL an element of a loading kind asked for; `null` for anything else. */
export function elementUrl(target: unknown): string | null {
  if (typeof target !== 'object' || target === null) return null;
  const element = target as Record<string, unknown>;
  if (typeof element.tagName !== 'string' || !LOADING_TAGS.includes(element.tagName.toUpperCase())) return null;
  for (const key of ['currentSrc', 'src', 'href']) {
    const value = element[key];
    if (typeof value === 'string' && value !== '') return value;
  }
  return null;
}

/** The origins a page failed to load from: each once, never the page's own, at most the cap. */
export class MissedOrigins {
  private readonly seen: string[] = [];

  constructor(
    private readonly own: string,
    private readonly report: (origin: string, via?: string) => void,
  ) {}

  /** `url` (against `base`) failed to load, asked for `via`. */
  note(url: string, base: string, via?: string): void {
    const origin = originOf(url, base);
    if (origin === null || origin === this.own || this.seen.includes(origin)) return;
    if (this.seen.length >= MAX_MISSED_ORIGINS) return;
    this.seen.push(origin);
    this.report(origin, via !== undefined && VIA_WORD.test(via) ? via : undefined);
  }
}

/** The part of a resource timing entry used here (`responseStatus` is missing in some engines). */
export interface ResourceEntryLike {
  name: string;
  responseStatus?: number;
  initiatorType?: string;
}

/**
 * Whether a resource entry tells of a miss: a loopback origin (always one on a device) or an
 * error status. A status of 0 alone is no evidence: an opaque cross-origin answer reports it too.
 */
export function isMissedEntry(entry: ResourceEntryLike, base: string): boolean {
  const origin = originOf(entry.name, base);
  if (origin === null) return false;
  return isLoopbackOrigin(origin) || (typeof entry.responseStatus === 'number' && entry.responseStatus >= 400);
}

/** The URLs (with their element) of `doc`'s elements that name a loopback origin, and of its broken images. */
export function scanDocument(doc: Document): Array<[string, string]> {
  const urls: Array<[string, string]> = [];
  for (const element of Array.from(doc.querySelectorAll(SCANNED))) {
    const url = elementUrl(element);
    if (url === null) continue;
    const origin = originOf(url, doc.baseURI);
    const image = element as Partial<HTMLImageElement>;
    const broken = element.tagName === 'IMG' && image.complete === true && image.naturalWidth === 0;
    if (broken || (origin !== null && isLoopbackOrigin(origin))) urls.push([url, element.tagName.toLowerCase()]);
  }
  return urls;
}

/** What a window needs for the watch. */
export interface MissWindow {
  document: Document;
  addEventListener(type: string, listener: (event: Event) => void, capture?: boolean): void;
  PerformanceObserver?: typeof PerformanceObserver;
  fetch?: typeof fetch;
  XMLHttpRequest?: typeof XMLHttpRequest;
}

/** What the watch hears: a URL that failed and how the page asked for it. */
export type NoteMiss = (url: string, via: string) => void;

/** Watches `win` for resources that fail to load; `note` hears each URL, perhaps more than once. */
export function watchMisses(win: MissWindow, note: NoteMiss): void {
  win.addEventListener(
    'error',
    (event) => {
      const url = elementUrl(event.target);
      const tag = (event.target as { tagName?: unknown } | null)?.tagName;
      if (url !== null && typeof tag === 'string') note(url, tag.toLowerCase());
    },
    true,
  );
  const scan = (): void => {
    for (const [url, via] of scanDocument(win.document)) note(url, via);
  };
  scan();
  win.addEventListener('load', scan);
  observeResources(win, note);
  wrapFetch(win, note);
  wrapXhr(win, note);
}

function observeResources(win: MissWindow, note: NoteMiss): void {
  const Observer = win.PerformanceObserver;
  if (typeof Observer !== 'function') return;
  try {
    new Observer((list) => {
      for (const entry of list.getEntries()) {
        const resource = entry as ResourceEntryLike;
        if (isMissedEntry(resource, win.document.baseURI)) note(resource.name, resource.initiatorType ?? 'other');
      }
    }).observe({ type: 'resource', buffered: true });
  } catch {
    // An engine without resource timing: the other signals remain.
  }
}

/** The URL a `fetch` asked for. */
function requestUrl(input: RequestInfo | URL): string {
  if (typeof input === 'string') return input;
  const request = input as { url?: unknown };
  return typeof request.url === 'string' ? request.url : String(input);
}

/** Wraps `fetch`: the page gets its own promise back; a rejection that is no abort is a miss. */
function wrapFetch(win: MissWindow, note: NoteMiss): void {
  const original = win.fetch;
  if (typeof original !== 'function') return;
  win.fetch = (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const answer = original.call(win, input, init);
    answer.catch((error: unknown) => {
      if ((error as { name?: unknown } | null)?.name !== 'AbortError') note(requestUrl(input), 'fetch');
    });
    return answer;
  };
}

/** Wraps `XMLHttpRequest.prototype.open`: an `error` of the request is a miss. */
function wrapXhr(win: MissWindow, note: NoteMiss): void {
  const proto = win.XMLHttpRequest?.prototype as { open?: unknown } | undefined;
  const open = proto?.open;
  if (!proto || typeof open !== 'function') return;
  proto.open = function (this: EventTarget, ...args: unknown[]): unknown {
    const url = String(args[1]);
    this.addEventListener('error', () => note(url, 'xmlhttprequest'), { once: true });
    return Reflect.apply(open, this, args);
  };
}
