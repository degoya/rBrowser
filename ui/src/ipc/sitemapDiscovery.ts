// The sitemap capture's start page (RB-099-09), mirror of rb-ipc's `sitemap_discovery.rs`; kept
// beside `protocol.ts`, which re-exports it. `sitemapStart` `discover` finds the project's pages
// without capturing one and is answered with `{ type: 'sitemapDiscovery', ...discovery }`;
// `close` closes the page's window (`ack`).

/** One node of the page tree: a site (its origin) at the top, a path segment below it, a page at
 * the end. Its pages are `pages[first .. first + count]`: the list is in the tree's order, so a
 * subtree is one range. A node without children is one page. */
export interface SitemapTreeNode {
  name: string;
  first: number;
  count: number;
  children?: SitemapTreeNode[];
}

/** How many Chromium breakpoints a run takes with each choice of views (RB-099-08). */
export interface SitemapViewCounts {
  all: number;
  visible: number;
}

/** The pages found, before anything is captured. */
export interface SitemapDiscovery {
  /** Names this result for `sitemap.capture`'s `discovery`. */
  discoveryId: string;
  /** The source as `run.json` names it. */
  source: string;
  /** The site's origin, when the source had one. */
  site?: string;
  /** The pages, in the tree's order. */
  pages: string[];
  tree: SitemapTreeNode[];
  /** A limit cut the discovery short. */
  truncated: boolean;
  /** The most pages one run takes. */
  maxPages: number;
  views: SitemapViewCounts;
}

export type SitemapStartShellMessage = { type: 'sitemapStart'; op: 'discover' | 'close' };
export type SitemapStartCoreMessage = { type: 'sitemapDiscovery' } & SitemapDiscovery;
