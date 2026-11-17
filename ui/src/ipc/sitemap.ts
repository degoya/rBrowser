// The last sitemap capture of the window (RB-050-05), mirror of rb-ipc's `SitemapStatus`; kept
// beside `protocol.ts`, which re-exports it.

/** Where a sitemap capture stands. */
export type SitemapRunState = 'running' | 'cancelled' | 'finished';

/** A sitemap capture's progress: the toolbar's indicator and the sitemap bar. */
export interface SitemapStatus {
  runId: string;
  state: SitemapRunState;
  /** Pages times breakpoints. */
  total: number;
  done: number;
  failed: number;
  /** The estimated seconds left; absent before the first shot and once it ended. */
  etaSeconds?: number;
  /** The run's folder. */
  folder: string;
  /** The gallery (`index.html`), once written. */
  gallery?: string;
}
