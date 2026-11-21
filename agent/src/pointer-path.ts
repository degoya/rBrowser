// The pointer's way while a recording runs (RB-090-87): the moves are sampled - a point only
// when it lies at least `PATH_MIN_DISTANCE_PX` from the last one and at least
// `PATH_MIN_INTERVAL_MS` after it -, the way keeps at most `PATH_MAX_POINTS` points of the last
// `PATH_MAX_MS`, the oldest dropped first: the end of a way, at the menu, matters most. A step
// takes the way that led to it; the way starts anew after it. Points are CSS px of the viewport,
// rounded, with `t` the ms after the way's first point.

import type { PathPoint } from './protocol';

/** The least distance between two points of a way, in CSS px. */
export const PATH_MIN_DISTANCE_PX = 8;
/** The least time between two points of a way, in ms. */
export const PATH_MIN_INTERVAL_MS = 16;
/** The most points one step's way keeps. */
export const PATH_MAX_POINTS = 64;
/** The longest way one step keeps, in ms before its end. */
export const PATH_MAX_MS = 10_000;

interface Sample {
  x: number;
  y: number;
  at: number;
}

/** The sampled way since the last step. */
export class PointerPath {
  private points: Sample[] = [];

  /** The pointer moved to (`x`, `y`) at `at` ms. */
  add(x: number, y: number, at: number): void {
    const point = { x: Math.round(x), y: Math.round(y), at };
    const last = this.points.at(-1);
    if (last) {
      if (point.at - last.at < PATH_MIN_INTERVAL_MS) return;
      if (Math.hypot(point.x - last.x, point.y - last.y) < PATH_MIN_DISTANCE_PX) return;
    }
    this.points.push(point);
    this.trim(at);
  }

  /** The way ending at (`x`, `y`), where the pointer was at `at`; the way starts anew. A way of
   *  fewer than two points is none (empty). */
  take(x: number, y: number, at: number): PathPoint[] {
    const end = { x: Math.round(x), y: Math.round(y), at };
    const last = this.points.at(-1);
    if (last && (last.x !== end.x || last.y !== end.y)) {
      this.points.push({ ...end, at: Math.max(end.at, last.at) });
    }
    this.trim(at);
    const points = this.points;
    this.points = [];
    const first = points[0];
    if (!first || points.length < 2) return [];
    let previous = 0;
    return points.map((point) => {
      // Never backwards, though the clock is.
      previous = Math.max(previous, point.at - first.at);
      return { x: point.x, y: point.y, t: previous };
    });
  }

  /** Forgets the way. */
  clear(): void {
    this.points = [];
  }

  private trim(now: number): void {
    const since = now - PATH_MAX_MS;
    let drop = 0;
    while (drop < this.points.length && (this.points[drop]?.at ?? now) < since) drop += 1;
    drop = Math.max(drop, this.points.length - PATH_MAX_POINTS);
    if (drop > 0) this.points.splice(0, drop);
  }
}
