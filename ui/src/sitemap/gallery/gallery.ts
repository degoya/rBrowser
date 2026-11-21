// The sitemap gallery page's logic (RB-090-86): the run its route names, the filters by
// breakpoint and by status, the order the large view steps through, and the list of problems.
// The page (`SitemapGalleryPage.vue`) only draws what this decides.

import type { GalleryBreakpoint, GalleryPage, SitemapGalleryPage } from '../../ipc/protocol';

/** The route of the page in the shell's pages. */
export const GALLERY_ROUTE = 'sitemap-gallery';

/** A run id as `rb_visual` writes it: a plain name. */
const RUN_ID = /^[A-Za-z0-9_-][A-Za-z0-9._-]{0,119}$/;

/** The run `#/sitemap-gallery?run=<id>` names; `null` without one or for one that is no plain name. */
export function runOfHash(hash: string): string | null {
  const query = hash.split('?', 2)[1];
  if (query === undefined) {
    return null;
  }
  const run = new URLSearchParams(query).get('run');
  return run !== null && RUN_ID.test(run) && !run.endsWith('.') ? run : null;
}

/** What the status filter keeps. */
export type StatusFilter = 'all' | 'ok' | 'problems';

/** The filters in effect: one breakpoint (or every one) and a status. */
export interface GalleryFilter {
  breakpoint: string;
  status: StatusFilter;
}

export const ALL = 'all';

/** A breakpoint's label, its name when it has none. */
export const labelOf = (breakpoint: GalleryBreakpoint): string => breakpoint.label || breakpoint.name;

/** What went wrong with a page: a status of 400 or more, a redirect, a failed shot. */
export interface Problem {
  kind: 'http' | 'redirect' | 'failed';
  /** The status, the redirect's target or the breakpoint and the reason. */
  detail: string;
}

export function problemsOf(page: GalleryPage): Problem[] {
  const found: Problem[] = [];
  if (page.status !== undefined && page.status >= 400) {
    found.push({ kind: 'http', detail: String(page.status) });
  }
  if (page.finalUrl !== undefined && page.finalUrl !== page.url) {
    found.push({ kind: 'redirect', detail: page.finalUrl });
  }
  for (const [breakpoint, shot] of Object.entries(page.shots)) {
    if (shot.state === 'failed') {
      found.push({ kind: 'failed', detail: `${breakpoint}: ${shot.error ?? ''}` });
    }
  }
  return found;
}

/** The breakpoints the sheet shows. */
export function shownBreakpoints(run: SitemapGalleryPage, filter: GalleryFilter): GalleryBreakpoint[] {
  return filter.breakpoint === ALL ? run.breakpoints : run.breakpoints.filter((bp) => bp.name === filter.breakpoint);
}

/** The pages the sheet shows, with their index in the run. */
export function shownPages(run: SitemapGalleryPage, filter: GalleryFilter): { page: GalleryPage; index: number }[] {
  return run.pages
    .map((page, index) => ({ page, index }))
    .filter(({ page }) => {
      if (filter.status === ALL) return true;
      const problem = problemsOf(page).length > 0;
      return filter.status === 'problems' ? problem : !problem;
    });
}

/** A shot the large view can show: its page's index and its breakpoint. */
export interface ShotRef {
  page: number;
  breakpoint: string;
}

/** Every shot with an image the sheet shows, row by row: the large view's order. */
export function shotOrder(run: SitemapGalleryPage, filter: GalleryFilter): ShotRef[] {
  const breakpoints = shownBreakpoints(run, filter);
  return shownPages(run, filter).flatMap(({ page, index }) =>
    breakpoints.filter((bp) => page.shots[bp.name]?.image !== undefined).map((bp) => ({ page: index, breakpoint: bp.name })),
  );
}

/** The shot `delta` steps from `current` in `order`; `null` past either end. */
export function step(order: ShotRef[], current: ShotRef, delta: number): ShotRef | null {
  const at = order.findIndex((shot) => shot.page === current.page && shot.breakpoint === current.breakpoint);
  if (at < 0) return null;
  return order[at + delta] ?? null;
}

/** A comparison's share of differing pixels in percent, two decimals. */
export const percentOf = (ratio: number): string => (ratio * 100).toFixed(2);
