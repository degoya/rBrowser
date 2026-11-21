// The sitemap gallery page of the app's gallery window (RB-090-86), mirror of rb-ipc's
// `sitemap_gallery.rs`; kept beside `protocol.ts`, which re-exports it. `getSitemapGallery` is
// answered with `{ type: 'sitemapGallery', ...page }`; a shot's image is an
// `rbrowser://sitemap/<run>/<slug>/<breakpoint>.png` address only the gallery window may load.

/** A breakpoint of the run. */
export interface GalleryBreakpoint {
  /** The file name of its shots (`mobile`). */
  name: string;
  /** Its label; empty: the name. */
  label: string;
  width: number;
  height: number;
}

/** A shot's comparison with its baseline. */
export interface GalleryCompare {
  /** `ok`, `differs`, `new`, `sizeMismatch` or `failed`. */
  status: string;
  /** The share of pixels that differ, 0 to 1. */
  ratio: number;
}

/** One page in one breakpoint. */
export interface GalleryShot {
  state: 'pending' | 'done' | 'failed' | string;
  error?: string;
  /** Its image on the gallery window's route; only for a shot that is done. */
  image?: string;
  compare?: GalleryCompare;
}

/** One page of the run. */
export interface GalleryPage {
  url: string;
  status?: number;
  /** Where a redirect ended. */
  finalUrl?: string;
  /** Its shots by breakpoint name. */
  shots: Record<string, GalleryShot>;
}

/** The run as the gallery page shows it. */
export interface SitemapGalleryPage {
  runId: string;
  source: string;
  state: 'running' | 'cancelled' | 'finished' | string;
  /** How wide a thumbnail is, in CSS px (`sitemap-capture.thumbnailWidth`). */
  thumbnailWidth: number;
  breakpoints: GalleryBreakpoint[];
  /** The first pages of the run, at most the core's limit. */
  pages: GalleryPage[];
  /** How many pages the run has, when `pages` holds fewer. */
  totalPages?: number;
}

export type SitemapGalleryRequest = { type: 'getSitemapGallery'; runId?: string };
export type SitemapGalleryEvent = { type: 'sitemapGallery' } & SitemapGalleryPage;
