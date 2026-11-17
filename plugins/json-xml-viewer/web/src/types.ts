// The settings the component hands the page script (`{"type": "config"}`), with the schema's
// defaults (plugins/json-xml-viewer/settings.schema.json) for a missing or malformed value.

export type ViewMode = 'tree' | 'raw' | 'pretty';
export type Theme = 'auto' | 'light' | 'dark';

export interface Config {
  /** `json-xml-viewer.enabled`: whether a JSON or XML document is shown as a tree at all. */
  enabled: boolean;
  /** `json-xml-viewer.maxBytes`: above this size the document stays raw, with a notice. */
  maxBytes: number;
  /** `json-xml-viewer.defaultView`: what a document opens in. */
  defaultView: 'tree' | 'raw';
  /** `json-xml-viewer.expandDepth`: the levels open when a document opens. */
  expandDepth: number;
  /** `json-xml-viewer.theme`: `auto` follows the view's `prefers-color-scheme`. */
  theme: Theme;
}

export const DEFAULT_CONFIG: Config = {
  enabled: true,
  maxBytes: 20 * 1024 * 1024,
  defaultView: 'tree',
  expandDepth: 2,
  theme: 'auto',
};

/** The bounds of the schema; a value outside them is the default. */
export const MAX_BYTES_RANGE: readonly [number, number] = [65536, 256 * 1024 * 1024];
export const EXPAND_DEPTH_RANGE: readonly [number, number] = [0, 16];

function inRange(value: unknown, [low, high]: readonly [number, number]): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= low && value <= high;
}

/** The config out of the component's answer, each field checked on its own. */
export function readConfig(answer: unknown): Config {
  const raw = (answer as { config?: Record<string, unknown> } | null)?.config ?? {};
  const pick = <T>(value: unknown, allowed: readonly T[], fallback: T): T =>
    allowed.includes(value as T) ? (value as T) : fallback;
  return {
    enabled: typeof raw.enabled === 'boolean' ? raw.enabled : DEFAULT_CONFIG.enabled,
    maxBytes: inRange(raw.maxBytes, MAX_BYTES_RANGE) ? raw.maxBytes : DEFAULT_CONFIG.maxBytes,
    defaultView: pick(raw.defaultView, ['tree', 'raw'] as const, DEFAULT_CONFIG.defaultView),
    expandDepth: inRange(raw.expandDepth, EXPAND_DEPTH_RANGE)
      ? raw.expandDepth
      : DEFAULT_CONFIG.expandDepth,
    theme: pick(raw.theme, ['auto', 'light', 'dark'] as const, DEFAULT_CONFIG.theme),
  };
}
