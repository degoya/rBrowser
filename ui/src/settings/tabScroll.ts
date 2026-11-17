// The arithmetic of the one tab row of the pages window (`TabStrip.vue`, RB-080-81): which sides
// hold more tabs, where the row scrolls to bring the chosen tab into sight, and how far a scroll
// button pages. Pure, so the narrow and the wide window are tested without a layout.

/** The sides of the row with tabs beyond them. */
export interface Edges {
  start: boolean;
  end: boolean;
}

/** The space kept beside a revealed tab, so its neighbour shows there are more. */
export const REVEAL_MARGIN = 24;
/** A scroll button pages by this share of the visible width, so one tab stays for orientation. */
export const PAGE_SHARE = 0.8;

/** Below a pixel is rounding (zoom, fractional widths), not a hidden tab. */
const TOLERANCE = 1;

export function overflowEdges(scrollLeft: number, scrollWidth: number, clientWidth: number): Edges {
  const rest = scrollWidth - clientWidth - scrollLeft;
  return { start: scrollLeft >= TOLERANCE, end: rest >= TOLERANCE };
}

function clamp(offset: number, scrollWidth: number, clientWidth: number): number {
  return Math.max(0, Math.min(offset, Math.max(0, scrollWidth - clientWidth)));
}

/** The `scrollLeft` that shows `tab` (its offset and width in the row): unchanged when it is in
 * sight, else just enough to show it with the margin; a tab wider than the visible part shows
 * its start. */
export function revealOffset(
  tab: { left: number; width: number },
  scrollLeft: number,
  clientWidth: number,
  scrollWidth: number,
): number {
  const end = tab.left + tab.width;
  let next = scrollLeft;
  if (tab.width + REVEAL_MARGIN >= clientWidth) {
    next = tab.left;
  } else if (tab.left < scrollLeft) {
    next = tab.left - REVEAL_MARGIN;
  } else if (end > scrollLeft + clientWidth) {
    next = end - clientWidth + REVEAL_MARGIN;
  }
  return clamp(next, scrollWidth, clientWidth);
}

/** The `scrollLeft` after one page towards `direction` (1 forward, -1 back). */
export function pageOffset(scrollLeft: number, clientWidth: number, scrollWidth: number, direction: 1 | -1): number {
  return clamp(scrollLeft + direction * Math.round(clientWidth * PAGE_SHARE), scrollWidth, clientWidth);
}
