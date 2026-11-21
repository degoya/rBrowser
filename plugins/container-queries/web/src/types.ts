// The shapes the page script, the analysis the component evaluates (assets/analyze.js) and the
// panel share. A view's report goes to the component as JSON (src/analysis.rs reads it).

import type { Bound } from './query';

/** One `@container` condition of the page, in the order of its stylesheets. */
export interface ConditionInfo {
  /** The container name the rule asks for; empty for the nearest container. */
  name: string;
  /** The condition as written, without the name. */
  query: string;
  bounds: Bound[];
  /** False for a condition this plugin cannot evaluate (a style query, an unknown unit). */
  supported: boolean;
}

/** One element with `container-type` of `size` or `inline-size`. */
export interface ContainerInfo {
  /** A selector that finds the element again, the same in every view of the same page. */
  path: string;
  /** A short name for people: the tag with its id or first classes. */
  selector: string;
  /** Its `container-name`s. */
  names: string[];
  /** Its `container-type`. */
  type: string;
  /** Its content box, in px. */
  inline: number;
  block: number;
  /** The conditions (indices into `conditions`) whose rules query this container. */
  conditions: number[];
  /** Those that match now. */
  matching: number[];
  /** The px of their bounds for this container, ascending, per axis. */
  thresholds: { inline: number[]; block: number[] };
  /** Whether rBrowser set its width (a drag or `containers_resize`). */
  resized: boolean;
}

/** One view's analysis. */
export interface Report {
  containers: ContainerInfo[];
  conditions: ConditionInfo[];
  /** Stylesheets the page script cannot read (another origin without CORS). */
  unreadableSheets: number;
  /** Whether a limit cut the containers or the conditions short. */
  truncated: boolean;
}

/** The page script's bridge (docs/plugins.md, "Page scripts"). */
export interface RbPlugin {
  readonly id: string;
  readonly view: number;
  readonly maxMessageBytes: number;
  readonly layers: { add(host: Element): boolean; remove(host: Element): void };
  send(message: unknown): Promise<unknown>;
  emit(message: unknown): void;
  on(listener: (message: unknown) => void): () => void;
  onStop(listener: () => void): () => void;
}

/** The settings the page script draws with (`container-queries.overlay.*`). */
export interface PageSettings {
  labels: boolean;
  color: string;
}

export const DEFAULT_SETTINGS: PageSettings = { labels: true, color: '#0d9488' };

/** `value` read as page settings, every bad field its default. */
export function pageSettings(value: unknown): PageSettings {
  const object = value && typeof value === 'object' ? (value as Record<string, unknown>) : {};
  const color = typeof object.color === 'string' && /^#[0-9a-fA-F]{6}$/.test(object.color) ? object.color : DEFAULT_SETTINGS.color;
  const labels = typeof object.labels === 'boolean' ? object.labels : DEFAULT_SETTINGS.labels;
  return { labels, color };
}
