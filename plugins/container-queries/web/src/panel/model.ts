// The panel's logic, apart from Vue: the analysis `containers_analyze` answers (src/analysis.rs)
// read defensively, the texts of conditions, bounds and sizes, the breakpoints that switch a
// container, and the events that ask for a new analysis.
import { isObject } from '../../../../web-common/src/is-object';

export interface Bound {
  axis: string;
  op: string;
  value: string;
  px: number | null;
}
export interface Condition {
  name: string;
  query: string;
  bounds: Bound[];
  supported: boolean;
}
export interface AxisSize {
  size: number;
  below: number | null;
  above: number | null;
}
export interface ContainerView {
  view: number;
  label: string;
  breakpoint: string;
  inline: AxisSize;
  block: AxisSize;
  matching: number[];
  resized: boolean;
}
export interface Switch {
  from: number;
  to: number;
  gained: number[];
  lost: number[];
}
export interface Container {
  path: string;
  selector: string;
  names: string[];
  type: string;
  conditions: number[];
  views: ContainerView[];
  switches: Switch[];
}
export interface ViewSummary {
  view: number;
  label: string;
  breakpoint: string;
  width: number;
  containers: number;
  unreadableSheets: number;
  truncated: boolean;
  error?: string;
}
export interface Analysis {
  conditions: Condition[];
  containers: Container[];
  views: ViewSummary[];
  /** Whether the overlay shows in the views analysed. */
  overlay: boolean;
}

type Json = Record<string, unknown>;
const text = (value: unknown): string => (typeof value === 'string' ? value : '');
const number = (value: unknown): number | null => (typeof value === 'number' && Number.isFinite(value) ? value : null);
const numbers = (value: unknown): number[] => (Array.isArray(value) ? value.filter((item): item is number => number(item) !== null) : []);
const list = <T>(value: unknown, read: (item: Json) => T | null): T[] =>
  Array.isArray(value) ? value.flatMap((item) => (isObject(item) ? (read(item) ?? []) : [])) : [];

function readAxis(value: unknown): AxisSize {
  const axis = isObject(value) ? value : {};
  return { size: number(axis.size) ?? 0, below: number(axis.below), above: number(axis.above) };
}

function readCondition(item: Json): Condition {
  const bounds = list(item.bounds, (bound) => ({ axis: text(bound.axis), op: text(bound.op), value: text(bound.value), px: number(bound.px) }));
  return { name: text(item.name), query: text(item.query), bounds, supported: item.supported === true };
}

function readContainer(item: Json): Container | null {
  if (typeof item.path !== 'string') return null;
  const views = list(item.views, (view) =>
    number(view.view) === null
      ? null
      : {
          view: number(view.view) as number,
          label: text(view.label),
          breakpoint: text(view.breakpoint),
          inline: readAxis(view.inline),
          block: readAxis(view.block),
          matching: numbers(view.matching),
          resized: view.resized === true,
        },
  );
  const switches = list(item.switches, (entry) =>
    number(entry.from) === null || number(entry.to) === null
      ? null
      : { from: number(entry.from) as number, to: number(entry.to) as number, gained: numbers(entry.gained), lost: numbers(entry.lost) },
  );
  const names = Array.isArray(item.names) ? item.names.filter((name): name is string => typeof name === 'string') : [];
  return { path: item.path, selector: text(item.selector), names, type: text(item.type), conditions: numbers(item.conditions), views, switches };
}

function readView(item: Json): ViewSummary | null {
  const view = number(item.view);
  if (view === null) return null;
  const summary: ViewSummary = {
    view,
    label: text(item.label),
    breakpoint: text(item.breakpoint),
    width: number(item.width) ?? 0,
    containers: number(item.containers) ?? 0,
    unreadableSheets: number(item.unreadableSheets) ?? 0,
    truncated: item.truncated === true,
  };
  if (typeof item.error === 'string') summary.error = item.error;
  return summary;
}

/** The analysis in `value`, or null when it is none. */
export function readAnalysis(value: unknown): Analysis | null {
  if (!isObject(value) || !Array.isArray(value.conditions) || !Array.isArray(value.containers) || !Array.isArray(value.views)) return null;
  return {
    conditions: list(value.conditions, readCondition),
    containers: list(value.containers, readContainer),
    views: list(value.views, readView),
    overlay: value.overlay === true,
  };
}

/** A condition as written: its container name, then its query. */
export function conditionText(condition: Condition | undefined): string {
  if (!condition) return '';
  return condition.name ? `${condition.name} ${condition.query}` : condition.query;
}

const OPS: Record<string, string> = { '>=': '≥', '<=': '≤' };

/** A bound: its comparison and value, with its px when the value has another unit. */
export function boundText(bound: Bound): string {
  const op = OPS[bound.op] ?? bound.op;
  const px = bound.px !== null && bound.value !== `${bound.px}px` ? ` (${bound.px} px)` : '';
  return `${op} ${bound.value}${px}`;
}

/** A size in px, or a dash for none. */
export function sizeText(value: number | null): string {
  return value === null ? '–' : `${Math.round(value * 100) / 100}`;
}

/** The thresholds around a size: the next one below (shrinking past it switches a condition) and above. */
export function nextText(axis: AxisSize): string {
  const parts: string[] = [];
  if (axis.below !== null) parts.push(`↓ ${sizeText(axis.below)}`);
  if (axis.above !== null) parts.push(`↑ ${sizeText(axis.above)}`);
  return parts.join('  ');
}

/** The view's label, or its id. */
function viewLabel(analysis: Analysis, view: number): string {
  return analysis.views.find((entry) => entry.view === view)?.label || `#${view}`;
}

/** Between which views the container changes, and which conditions it gains and loses there. */
export function switchRows(container: Container, analysis: Analysis): Array<{ from: string; to: string; gained: string[]; lost: string[] }> {
  const names = (ids: number[]): string[] => ids.map((id) => conditionText(analysis.conditions[id]));
  return container.switches.map((entry) => ({ from: viewLabel(analysis, entry.from), to: viewLabel(analysis, entry.to), gained: names(entry.gained), lost: names(entry.lost) }));
}

/** An event of the page scripts or the component: a new analysis is due, or the overlay switched. */
export function readEvent(message: unknown): { refresh: true } | { overlay: boolean } | null {
  if (!isObject(message)) return null;
  if (message.type === 'refresh' || message.type === 'resized' || message.type === 'reset') return { refresh: true };
  if (message.type === 'overlay' && typeof message.on === 'boolean') return { overlay: message.on };
  return null;
}
