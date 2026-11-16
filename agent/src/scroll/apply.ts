// Scroll sync in a follower (RB-020-04): the leader's scroll event brings the same content to the
// same edge here. Per axis: at the very start or end of the leader's range, exactly the start or
// end here; else the anchor's point onto the edge of the visible area; without the anchor, the
// same share of the scroll range. The scroll is instant, so a page with `scroll-behavior:
// smooth` does not animate against the next frame's scroll. A container that is not found is
// `not_found` (the page itself keeps following); a missing anchor is not a miss.

import type { CommandResult, ElementTarget, ResolveResult, ScrollData } from '../protocol';
import { locate } from '../resolve';
import { positionOf, viewportOf, type Scroller } from './anchor';

/** Within this many px of either end of the range the leader is at that end. */
const EDGE = 1;

/**
 * The scroll offset for one axis here. `leader` and `leaderMax`: the leader's offset and range;
 * `current` and `max`: this view's; `delta`: how far the anchor's point is from the edge here
 * (CSS px, positive below or right of it), null without an anchor.
 */
export function targetAxis(
  leader: number,
  leaderMax: number,
  current: number,
  max: number,
  delta: number | null,
): number {
  if (max <= 0 || leader <= EDGE) return 0;
  if (leader >= leaderMax - EDGE) return max;
  const wanted = delta !== null ? current + delta : (leader / leaderMax) * max;
  return Math.round(Math.min(max, Math.max(0, wanted)));
}

/** The anchor's element in `scroller`, or null when it is missing or outside it. */
function anchorIn(anchor: ElementTarget, scroller: Scroller, doc: Document): Element | null {
  const found = locate(anchor, doc);
  if (typeof found === 'string') return null;
  if (scroller instanceof Element && !scroller.contains(found)) return null;
  return found;
}

/** Applies the leader's scroll `data` in `doc`. */
export function applyScroll(data: ScrollData, doc: Document): CommandResult {
  let scroller: Scroller = doc;
  if (data.container) {
    const found = locate(data.container, doc);
    if (typeof found === 'string') return { status: found };
    scroller = found;
  }
  const here = positionOf(scroller);
  let dx: number | null = null;
  let dy: number | null = null;
  const anchor = data.anchor ? anchorIn(data.anchor, scroller, doc) : null;
  if (anchor && data.anchor) {
    const rect = anchor.getBoundingClientRect();
    const area = viewportOf(scroller);
    dx = rect.left + data.anchor.offset.x * rect.width - area.left;
    dy = rect.top + data.anchor.offset.y * rect.height - area.top;
  }
  const left = targetAxis(data.x, data.maxX, here.x, here.maxX, dx);
  const top = targetAxis(data.y, data.maxY, here.y, here.maxY, dy);
  if (Math.abs(left - here.x) < 0.5 && Math.abs(top - here.y) < 0.5) return { status: 'done' };
  const options: ScrollToOptions = { left, top, behavior: 'instant' };
  if (scroller instanceof Element) scroller.scrollTo(options);
  else doc.defaultView?.scrollTo(options);
  return { status: 'done' };
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}

/**
 * Where a wheel for `target` goes in `doc` (wheel mode): the target's point, moved into the
 * viewport, never scrolling; the viewport's centre when the target is missing (the page then
 * takes the wheel).
 */
export function pointOf(target: ElementTarget, doc: Document): ResolveResult {
  const view = doc.defaultView;
  const width = view?.innerWidth ?? 0;
  const height = view?.innerHeight ?? 0;
  const found = locate(target, doc);
  let x = width / 2;
  let y = height / 2;
  if (typeof found !== 'string') {
    const rect = found.getBoundingClientRect();
    x = rect.left + target.offset.x * rect.width;
    y = rect.top + target.offset.y * rect.height;
  }
  const inside = (value: number, size: number) => Math.min(Math.max(0, value), Math.max(0, size - 1));
  return { status: 'ok', x: round(inside(x, width)), y: round(inside(y, height)), scrolled: false };
}
