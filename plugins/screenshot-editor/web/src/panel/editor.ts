// The screenshot editor's model (RB-080-01): the tools and their keys, an annotation from a drag
// or two keyboard points, and the undo history. No DOM: the session (`session.ts`) and the input
// (`input.ts`) use it; the component checks every annotation again (src/model.rs).
import type { ShotAnnotation, ShotOptions } from './types';

/** The tools in toolbar order, each with its key inside the panel. */
export const TOOLS: readonly (readonly [string, string])[] = Object.freeze([
  ['rect', 'r'],
  ['arrow', 'a'],
  ['line', 'l'],
  ['freehand', 'f'],
  ['text', 't'],
  ['marker', 'm'],
  ['highlight', 'h'],
  ['redact', 'b'],
  ['crop', 'c'],
] as const);
/** The undo steps kept (the job asks for at least 50). */
export const HISTORY = 100;
/** The longest text, as the component holds it. */
export const MAX_TEXT = 200;
/** Freehand points closer than this to the last kept one are dropped. */
const MIN_STEP = 2;
/** Tools that take two points; the others are placed at one. */
export const DRAGGED: ReadonlySet<string> = new Set(['rect', 'arrow', 'line', 'freehand', 'highlight', 'redact', 'crop']);

export const toolForKey = (key: string): string | null => TOOLS.find(([, k]) => k === key.toLowerCase())?.[0] ?? null;

const round = (value: number): number => Math.round(value * 10) / 10;

/** A rectangle from two corners in any order. */
function box(a: number[], b: number[]): { x: number; y: number; w: number; h: number } {
  const [ax, ay, bx, by] = [a[0] ?? 0, a[1] ?? 0, b[0] ?? 0, b[1] ?? 0];
  return { x: round(Math.min(ax, bx)), y: round(Math.min(ay, by)), w: round(Math.abs(bx - ax)), h: round(Math.abs(by - ay)) };
}

export function simplify(points: number[][]): number[][] {
  const kept: number[][] = [];
  for (const point of points) {
    const last = kept[kept.length - 1];
    if (!last || Math.hypot((point[0] ?? 0) - (last[0] ?? 0), (point[1] ?? 0) - (last[1] ?? 0)) >= MIN_STEP) {
      kept.push([round(point[0] ?? 0), round(point[1] ?? 0)]);
    }
  }
  const end = points[points.length - 1];
  const tail = kept[kept.length - 1];
  if (end && tail && points.length > 1 && (tail[0] !== round(end[0] ?? 0) || tail[1] !== round(end[1] ?? 0))) {
    kept.push([round(end[0] ?? 0), round(end[1] ?? 0)]);
  }
  return kept;
}

/** The next marker number. */
export const nextMarker = (items: readonly ShotAnnotation[]): number =>
  items.reduce((top, item) => (item.type === 'marker' ? Math.max(top, item.number ?? 0) : top), 0) + 1;

/**
 * The annotation `tool` makes from `points` (a drag's start and end, or the freehand path; one
 * point for text and markers); null when it would be empty.
 */
export function build(tool: string, points: number[][], options: ShotOptions, items: readonly ShotAnnotation[], text = ''): ShotAnnotation | null {
  const first = points[0];
  const last = points[points.length - 1];
  if (!first || !last) return null;
  const { color, width, size } = options;
  if (tool === 'freehand') {
    const path = simplify(points);
    return { type: 'freehand', points: path, color, width };
  }
  if (tool === 'text') {
    // Control characters become spaces, as the component holds them.
    const words = text.replace(/[\u0000-\u001f\u007f]/g, ' ').trim().slice(0, MAX_TEXT);
    return words ? { type: 'text', x: round(first[0] ?? 0), y: round(first[1] ?? 0), text: words, color, size } : null;
  }
  if (tool === 'marker') {
    return { type: 'marker', x: round(first[0] ?? 0), y: round(first[1] ?? 0), number: nextMarker(items), color, size };
  }
  if (tool === 'arrow' || tool === 'line') {
    const line = { x1: round(first[0] ?? 0), y1: round(first[1] ?? 0), x2: round(last[0] ?? 0), y2: round(last[1] ?? 0) };
    if (Math.hypot(line.x2 - line.x1, line.y2 - line.y1) < 1) return null;
    return { type: tool, ...line, color, width };
  }
  const rect = box(first, last);
  if (rect.w < 1 || rect.h < 1) return null;
  if (tool === 'rect') return { type: 'rect', ...rect, color, width, fill: options.fill };
  if (tool === 'highlight') return { type: 'highlight', ...rect, color };
  if (tool === 'redact') return { type: 'redact', ...rect, mode: options.mode, block: options.block };
  if (tool === 'crop') return { type: 'crop', ...rect };
  return null;
}

/** `items` with `annotation` added; a new crop replaces the old one. */
export const add = (items: readonly ShotAnnotation[], annotation: ShotAnnotation): ShotAnnotation[] =>
  annotation.type === 'crop' ? [...items.filter((item) => item.type !== 'crop'), annotation] : [...items, annotation];

export interface History {
  readonly items: ShotAnnotation[];
  readonly canUndo: boolean;
  readonly canRedo: boolean;
  push(items: ShotAnnotation[]): void;
  undo(): ShotAnnotation[];
  redo(): ShotAnnotation[];
  reset(items: ShotAnnotation[]): void;
}

/** The undo history: every state of the annotation list, at most `limit` back. */
export function history(limit = HISTORY): History {
  let past: ShotAnnotation[][] = [];
  let future: ShotAnnotation[][] = [];
  let current: ShotAnnotation[] = [];
  return {
    get items() {
      return current;
    },
    get canUndo() {
      return past.length > 0;
    },
    get canRedo() {
      return future.length > 0;
    },
    push(items) {
      past.push(current);
      if (past.length > limit) past = past.slice(past.length - limit);
      future = [];
      current = items;
    },
    undo() {
      const previous = past.pop();
      if (previous) {
        future.push(current);
        current = previous;
      }
      return current;
    },
    redo() {
      const next = future.pop();
      if (next) {
        past.push(current);
        current = next;
      }
      return current;
    },
    reset(items) {
      past = [];
      future = [];
      current = items;
    },
  };
}

/** Whether the call carrying `args` stays below a panel message's 64 KiB. */
export const fits = (args: unknown): boolean => new TextEncoder().encode(JSON.stringify({ type: 'call', id: 0, tool: 'annotate', args })).length < 64 * 1024;
