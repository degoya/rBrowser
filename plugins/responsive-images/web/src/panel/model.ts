// The responsive images panel's model (RB-093-02), without DOM: the answer of the tool `check`
// (one entry per view, each image measured at the view's density), the tabs - one per view, the
// breakpoint and its density, the problems as a count -, the rows of a view's table, the marker
// that highlights a hovered image in every view, and the one status line. App.vue and
// ImageTable.vue draw it. Texts are catalogue messages through the shared texts.js.

// The shared texts.js is the panel's prelude (panel-ui); imported here too for the tests.
import '../../../../web-common/panel/texts.js';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import type { ChosenSource, FileFrom, Size } from '../types';

export const PREFIX = 'plugin.responsive-images.';
export type Strings = Record<string, string>;
export type Severity = 'error' | 'warning' | 'info';

export interface PanelIssue {
  rule: string;
  variant?: string | null;
  severity: Severity;
  /** The full message key. */
  message: string;
  params: Record<string, string>;
}

export interface PanelImage {
  selector: string;
  currentSrc: string;
  srcset?: string;
  sizes: string;
  picture: boolean;
  source: ChosenSource | null;
  loading: string;
  fetchpriority: string;
  rendered: Size;
  file: Size | null;
  fileFrom: FileFrom | null;
  needed: Size | null;
  ratio: number | null;
  vector: boolean;
  loaded: boolean;
  visible: boolean;
  lcp: boolean;
  issues: PanelIssue[];
}

export interface ViewAnswer {
  id: number;
  label: string;
  breakpoint: string;
  engine: string;
  width: number;
  height: number;
  dpr: number;
  url?: string;
  images?: PanelImage[];
  more?: number;
  lcpKnown?: boolean;
  /** Images without a problem left out of a shortened answer. */
  hidden?: number;
  error?: { code: string; params?: Record<string, string> };
}

export interface Answer {
  views: ViewAnswer[];
  summary: Record<Severity, number>;
  settings: { oversizeFactor: number; undersizeFactor: number; checkLcp: boolean };
  cut: boolean;
}

/** The icon and the colour of each severity, beside its word. */
export const SEVERITY_ICON: Record<Severity, string> = {
  error: 'i-lucide-circle-x',
  warning: 'i-lucide-triangle-alert',
  info: 'i-lucide-info',
};
export const SEVERITY_COLOR = { error: 'error', warning: 'warning', info: 'info' } as const;

/** The text `key` below `plugin.responsive-images.` with `{name}` filled in; the key when missing. */
export function text(strings: Strings, key: string, params: Record<string, string | number> = {}): string {
  return PanelTexts.format(strings[PREFIX + key] ?? key, params);
}

/** An issue's sentence: its message (a full key) with its params. */
export function issueText(strings: Strings, issue: PanelIssue): string {
  return PanelTexts.format(strings[issue.message] ?? issue.message, issue.params);
}

/** A number with at most two decimals. */
export function num(value: number): string {
  return String(Math.round(value * 100) / 100);
}

export function sizeText(size: Size): string {
  return `${num(size.width)} × ${num(size.height)}`;
}

/** The file name of an image URL: its last path segment, decoded; `data:` URIs by their type. */
export function fileName(url: string): string {
  if (url.startsWith('data:')) return url.slice(0, url.search(/[;,]/) === -1 ? 30 : url.search(/[;,]/));
  try {
    const parsed = new URL(url);
    const last = parsed.pathname.split('/').filter(Boolean).at(-1) ?? parsed.host;
    return decodeURIComponent(last);
  } catch {
    return url.split(/[?#]/)[0]?.split('/').at(-1) ?? url;
  }
}

/** One view's tab: its label, its density and its count of problems. */
export interface Tab {
  value: string;
  /** The tab's content slot (`view-<id>`). */
  slot: string;
  label: string;
  badge?: string;
  icon: string;
  view: ViewAnswer;
}

/** The number of problems of `view`. */
export function problemsOf(view: ViewAnswer): number {
  return (view.images ?? []).reduce((count, image) => count + image.issues.length, 0);
}

export function tabsOf(strings: Strings, answer: Answer | null): Tab[] {
  return (answer?.views ?? []).map((view) => ({
    value: String(view.id),
    slot: `view-${view.id}`,
    label: text(strings, 'panel.tab', { label: view.label, dpr: num(view.dpr) }),
    badge: view.error ? undefined : String(problemsOf(view)),
    icon: view.error ? 'i-lucide-circle-slash' : problemsOf(view) > 0 ? 'i-lucide-triangle-alert' : 'i-lucide-circle-check',
    view,
  }));
}

/** Why an image is listed but not measured; null when it is measured. */
export function unmeasured(image: PanelImage): 'hidden' | 'pending' | 'vector' | 'tiny' | null {
  if (!image.visible) return 'hidden';
  if (image.vector) return 'vector';
  if (!image.loaded || !image.file) return 'pending';
  if (image.needed === null) return 'tiny';
  return null;
}

/** One row of a view's table. */
export interface Row {
  image: PanelImage;
  name: string;
  /** Where the file came from: the picture's source (with its media), the srcset or src. */
  origin: string;
  rendered: string;
  needed: string;
  file: string;
  ratio: string;
  /** The image's own state where it is not measured. */
  note: string | null;
  /** `loading`, `fetchpriority` and LCP, as short words. */
  flags: string[];
}

export function rowsOf(strings: Strings, view: ViewAnswer, onlyProblems: boolean): Row[] {
  const images = (view.images ?? []).filter((image) => !onlyProblems || image.issues.length > 0);
  return images.map((image) => {
    const why = unmeasured(image);
    const origin = image.source
      ? text(strings, image.source.media ? 'panel.origin.sourceMedia' : 'panel.origin.source', { media: image.source.media })
      : text(strings, image.srcset ? 'panel.origin.srcset' : 'panel.origin.src');
    const flags = [
      ...(image.lcp ? [text(strings, 'panel.flag.lcp')] : []),
      ...(image.loading === 'lazy' ? [text(strings, 'panel.flag.lazy')] : []),
      ...(image.fetchpriority === 'high' ? [text(strings, 'panel.flag.high')] : []),
    ];
    return {
      image,
      name: fileName(image.currentSrc),
      origin,
      rendered: sizeText(image.rendered),
      needed: image.needed ? sizeText(image.needed) : '',
      file: image.file && !image.vector ? sizeText(image.file) : '',
      ratio: image.ratio === null ? '' : `${num(image.ratio)}×`,
      note: why ? text(strings, `panel.unmeasured.${why}`) : null,
      flags,
    };
  });
}

/** The targets of a mark: `selector` in every view that was measured. */
export function markTargets(answer: Answer | null, selector: string): { view: number; selector: string }[] {
  return (answer?.views ?? []).filter((view) => !view.error).map((view) => ({ view: view.id, selector }));
}

/**
 * The hover's marker: `hover(selector)` marks the image in every view after `delay` ms, a
 * hover elsewhere within that time replaces it, `hover(null)` clears the marks; one call at a
 * time - a hover during a call waits for it, only the latest.
 */
export function createMarker(send: (selector: string | null) => Promise<unknown>, delay = 150): { hover(selector: string | null): void; stop(): void } {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let busy = false;
  let pending: { selector: string | null } | null = null;
  let shown: string | null = null;
  const flush = (): void => {
    if (busy || !pending) return;
    const { selector } = pending;
    pending = null;
    if (selector === shown) return;
    busy = true;
    shown = selector;
    send(selector)
      .catch(() => undefined)
      .finally(() => {
        busy = false;
        flush();
      });
  };
  return {
    hover(selector) {
      clearTimeout(timer);
      timer = setTimeout(() => {
        pending = { selector };
        flush();
      }, delay);
    },
    stop() {
      clearTimeout(timer);
      pending = null;
    },
  };
}

/** The panel's one status line: a failure, the check on its way, or what the answer leaves out. */
export function statusOf(strings: Strings, state: { failure: string | null; loading: boolean; answer: Answer | null }): StatusLine | null {
  if (state.failure) return { tone: 'error', text: state.failure };
  if (state.loading) return { tone: 'busy', text: text(strings, 'panel.loading') };
  const answer = state.answer;
  if (!answer) return null;
  if (answer.cut) return { tone: 'warn', text: text(strings, 'panel.cut') };
  const { error, warning, info } = answer.summary;
  return { tone: error + warning + info > 0 ? 'warn' : 'ok', text: text(strings, 'panel.summary', answer.summary) };
}
