// The shapes the component, the page script and the panel share (src/tools.rs, src/report.rs,
// src/settings.rs of the plugin).

/** One overlay's state. */
export interface OverlayState {
  enabled: boolean;
  options?: Record<string, unknown>;
}

/** The state file: what every view shows and what single views show instead. */
export interface State {
  version?: number;
  all: Record<string, OverlayState>;
  views: Record<string, Record<string, OverlayState>>;
}

/** The settings the page script draws with. */
export interface PageSettings {
  mutationThrottleMs: number;
  show100vw: boolean;
  containersOnly: boolean;
  colors: { overflow: string; cause: string; vw100: string; highlight: string };
  /** The further overlays (RB-070-07). */
  targetSizePx: number;
  smallTextPx: number;
  scrollbarTolerancePx: number;
  chaosFactor: number;
  cssOffIncludeInline: boolean;
  /** The tab order overlay (RB-070-12). */
  tabOrderShowLines: boolean;
  tabOrderLive: boolean;
  tabOrderIncludeIframes: boolean;
}

/** A box in CSS px. */
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** An element a report names. */
export interface Mark {
  selector: string;
  rect: Rect;
  overhang?: number;
}

/** One view's overflow. */
export interface Report {
  hasHorizontalScroll: boolean;
  scrollWidth: number;
  clientWidth: number;
  innerWidth: number;
  causes: Mark[];
  vw100: Mark[];
}

/** The page script's bridge (docs/plugins.md "Page scripts"). */
export interface RbPlugin {
  readonly id: string;
  readonly view: number;
  readonly maxMessageBytes: number;
  send(message: unknown): Promise<unknown>;
  emit(message: unknown): void;
  on(listener: (message: unknown) => void): () => void;
  onStop(listener: () => void): () => void;
}

export const DEFAULT_SETTINGS: PageSettings = {
  mutationThrottleMs: 250,
  show100vw: true,
  containersOnly: false,
  colors: { overflow: '#f5b800', cause: '#e5262b', vw100: '#1f6feb', highlight: '#c026d3' },
  targetSizePx: 24,
  smallTextPx: 12,
  scrollbarTolerancePx: 4,
  chaosFactor: 2,
  cssOffIncludeInline: false,
  tabOrderShowLines: true,
  tabOrderLive: true,
  tabOrderIncludeIframes: true,
};

/** `value` when it is a number within `min..=max` (a whole one for `whole`), else `fallback`. */
export function within(value: unknown, min: number, max: number, fallback: number, whole = true): number {
  return typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max && (!whole || Number.isInteger(value))
    ? value
    : fallback;
}

const COLOR = /^#[0-9a-fA-F]{6}$/;

/** `value` as settings: every field checked, a missing or broken one its default. */
export function toSettings(value: unknown): PageSettings {
  const raw = (value && typeof value === 'object' ? value : {}) as Record<string, unknown>;
  const colors = (raw.colors && typeof raw.colors === 'object' ? raw.colors : {}) as Record<string, unknown>;
  const color = (key: keyof PageSettings['colors']): string => {
    const candidate = colors[key];
    return typeof candidate === 'string' && COLOR.test(candidate) ? candidate : DEFAULT_SETTINGS.colors[key];
  };
  const flag = (key: 'cssOffIncludeInline' | 'tabOrderShowLines' | 'tabOrderLive' | 'tabOrderIncludeIframes'): boolean =>
    typeof raw[key] === 'boolean' ? (raw[key] as boolean) : DEFAULT_SETTINGS[key];
  const throttle = raw.mutationThrottleMs;
  return {
    mutationThrottleMs:
      typeof throttle === 'number' && throttle >= 50 && throttle <= 5000 ? throttle : DEFAULT_SETTINGS.mutationThrottleMs,
    show100vw: typeof raw.show100vw === 'boolean' ? raw.show100vw : DEFAULT_SETTINGS.show100vw,
    containersOnly: typeof raw.containersOnly === 'boolean' ? raw.containersOnly : DEFAULT_SETTINGS.containersOnly,
    colors: { overflow: color('overflow'), cause: color('cause'), vw100: color('vw100'), highlight: color('highlight') },
    targetSizePx: within(raw.targetSizePx, 8, 96, DEFAULT_SETTINGS.targetSizePx),
    smallTextPx: within(raw.smallTextPx, 6, 32, DEFAULT_SETTINGS.smallTextPx),
    scrollbarTolerancePx: within(raw.scrollbarTolerancePx, 0, 100, DEFAULT_SETTINGS.scrollbarTolerancePx),
    chaosFactor: within(raw.chaosFactor, 0.1, 10, DEFAULT_SETTINGS.chaosFactor, false),
    cssOffIncludeInline: flag('cssOffIncludeInline'),
    tabOrderShowLines: flag('tabOrderShowLines'),
    tabOrderLive: flag('tabOrderLive'),
    tabOrderIncludeIframes: flag('tabOrderIncludeIframes'),
  };
}

/** What `view` shows of `overlay`: its own state, else every view's, else off (src/state.rs). */
export function effective(state: State | undefined, view: number, overlay: string): OverlayState {
  const own = state?.views?.[String(view)]?.[overlay];
  return own ?? state?.all?.[overlay] ?? { enabled: false };
}
