// The Web Vitals of the top document (RB-060-10), measured in the agent's isolated world with
// the engine's own `PerformanceObserver` - the DOM and the performance timeline are shared with
// the page, so the page sees nothing of it and no npm dependency rides in every frame. LCP with
// its element, CLS with its largest sources (`cls.ts`), INP with its target (`inp.ts`), FCP and
// TTFB. The observers are passive and buffered; a report goes out at most once per
// `REPORT_MS` while something changed, and at once when the page is hidden.
//
// A soft navigation - a new path or query through the history API, seen on `popstate` and the
// Navigation API's `currententrychange` - starts a new segment: CLS and INP start over, the
// load metrics (LCP, FCP, TTFB) belong to the document load and are left out.

import type { VitalMetric, VitalsData } from '../protocol';
import { cssPath } from '../selector';
import { ClsWindows } from './cls';
import { InpTracker } from './inp';

/** The least time between two reports, in ms. */
export const REPORT_MS = 1000;
/** The longest selector a report carries. */
const MAX_PATH = 512;
/** Event entries shorter than this are not delivered (web.dev's default threshold). */
const DURATION_THRESHOLD = 40;

/** The entry types each metric needs. */
const NEEDS: [VitalMetric, string][] = [
  ['lcp', 'largest-contentful-paint'],
  ['cls', 'layout-shift'],
  ['inp', 'event'],
  ['fcp', 'paint'],
  ['ttfb', 'navigation'],
];

/** The part of `PerformanceObserver` the collector uses. */
export interface ObserverClass {
  new (callback: (list: { getEntries(): PerformanceEntry[] }) => void): {
    observe(init: PerformanceObserverInit): void;
    disconnect(): void;
  };
  readonly supportedEntryTypes?: readonly string[];
}

/** How the collector is installed. */
export interface VitalsOptions {
  win: Window;
  emit: (data: VitalsData) => void;
  /** Default: the window's `PerformanceObserver`. */
  Observer?: ObserverClass;
}

/** The fields of the entry types read here; every one may be missing. */
interface Entry {
  name?: string;
  startTime?: number;
  duration?: number;
  value?: number;
  hadRecentInput?: boolean;
  sources?: { node?: Node | null }[];
  element?: Element | null;
  target?: Node | null;
  interactionId?: number;
  responseStart?: number;
  activationStart?: number;
}

/** The CSS path of `node` (its element for a text node), capped; undefined when there is none. */
function pathOf(node: Node | null | undefined): string | undefined {
  const element = node && (node.nodeType === 1 ? (node as Element) : node.parentElement);
  if (!element || !element.isConnected) return undefined;
  try {
    const path = cssPath(element);
    return path.length <= MAX_PATH ? path : undefined;
  } catch {
    return undefined;
  }
}

/** The route of a location: a change of it is a soft navigation, a new hash is not. */
function routeOf(win: Window): string {
  return `${win.location.pathname}${win.location.search}`;
}

function finite(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0;
}

/** Starts measuring in `win`; returns the function that stops it. */
export function installVitals(options: VitalsOptions): () => void {
  const { win, emit } = options;
  const Observer =
    options.Observer ?? (win as unknown as { PerformanceObserver?: ObserverClass }).PerformanceObserver;
  const supported = new Set(Observer?.supportedEntryTypes ?? []);
  const unsupported = NEEDS.filter(([, type]) => !supported.has(type)).map(([metric]) => metric);
  const cls = new ClsWindows();
  const inp = new InpTracker();
  let lcp: VitalsData['lcp'];
  let fcp: number | undefined;
  let ttfb: number | undefined;
  let segment = 0;
  let segmentStart = 0;
  let route = routeOf(win);
  let dirty = true;
  let timer: ReturnType<typeof setTimeout> | null = null;

  const interactionCount = (): number | undefined => {
    const count = (win.performance as unknown as { interactionCount?: unknown }).interactionCount;
    return finite(count) ? count : undefined;
  };

  const report = (): VitalsData => {
    const data: VitalsData = { segment, soft: segment > 0, url: win.location.href, unsupported };
    if (segment === 0) {
      if (lcp) data.lcp = lcp;
      if (fcp !== undefined) data.fcp = fcp;
      if (ttfb !== undefined) data.ttfb = ttfb;
    }
    if (!unsupported.includes('cls')) {
      const value = cls.value();
      data.cls = { value: Math.round(value.value * 10_000) / 10_000, sources: value.sources };
    }
    const interaction = inp.value(interactionCount());
    if (interaction) data.inp = interaction;
    return data;
  };

  const flush = (): void => {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    if (!dirty) return;
    dirty = false;
    emit(report());
  };

  const changed = (): void => {
    dirty = true;
    if (timer === null) timer = setTimeout(flush, REPORT_MS);
  };

  const checkRoute = (): void => {
    const now = routeOf(win);
    if (now === route) return;
    flush();
    route = now;
    segment += 1;
    segmentStart = win.performance.now();
    cls.reset();
    inp.reset();
    changed();
  };

  const handle = (type: string, entry: Entry): boolean => {
    const start = entry.startTime ?? 0;
    switch (type) {
      case 'largest-contentful-paint':
        if (segment > 0 || !finite(entry.startTime)) return false;
        lcp = { value: Math.round(entry.startTime) };
        {
          const element = pathOf(entry.element);
          if (element !== undefined) lcp.element = element;
        }
        return true;
      case 'layout-shift':
        if (start < segmentStart || !finite(entry.value)) return false;
        return cls.add({
          value: entry.value,
          startTime: start,
          hadRecentInput: entry.hadRecentInput === true,
          sources: (entry.sources ?? []).map((source) => pathOf(source.node)).filter((path) => path !== undefined),
        });
      case 'event':
      case 'first-input': {
        if (start < segmentStart || !finite(entry.duration)) return false;
        const target = pathOf(entry.target);
        return inp.add({
          interactionId: entry.interactionId ?? 0,
          duration: entry.duration,
          ...(target === undefined ? {} : { target }),
        });
      }
      case 'paint':
        if (entry.name !== 'first-contentful-paint' || !finite(entry.startTime)) return false;
        fcp = Math.round(entry.startTime);
        return true;
      case 'navigation': {
        const value = (entry.responseStart ?? 0) - (entry.activationStart ?? 0);
        if (!finite(value) || value <= 0) return false;
        ttfb = Math.round(value);
        return true;
      }
      default:
        return false;
    }
  };

  const observers: { disconnect(): void }[] = [];
  const types = [...NEEDS.map(([, type]) => type), 'first-input'].filter((type) => supported.has(type));
  for (const type of types) {
    if (!Observer) break;
    try {
      const observer = new Observer((list) => {
        let any = false;
        for (const entry of list.getEntries()) any = handle(type, entry as unknown as Entry) || any;
        if (any) changed();
      });
      const init: PerformanceObserverInit & { durationThreshold?: number } = { type, buffered: true };
      if (type === 'event') init.durationThreshold = DURATION_THRESHOLD;
      observer.observe(init);
      observers.push(observer);
    } catch {
      // The engine lists the type but refuses it here: that metric stays absent.
    }
  }

  const onHidden = (): void => {
    if (win.document.visibilityState === 'hidden') flush();
  };
  const navigation = (win as unknown as { navigation?: EventTarget }).navigation;
  win.addEventListener('popstate', checkRoute);
  navigation?.addEventListener('currententrychange', checkRoute);
  win.document.addEventListener('visibilitychange', onHidden);
  win.addEventListener('pagehide', flush);
  changed();

  return () => {
    for (const observer of observers) observer.disconnect();
    win.removeEventListener('popstate', checkRoute);
    navigation?.removeEventListener('currententrychange', checkRoute);
    win.document.removeEventListener('visibilitychange', onHidden);
    win.removeEventListener('pagehide', flush);
    if (timer !== null) clearTimeout(timer);
    timer = null;
  };
}
