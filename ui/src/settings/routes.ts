// The pages of the pages window by route (`#/<page>`). Each page gets the IPC client and the
// route's key: the settings, the trust dialog (RB-010-09, `trust`) and the analyser's wizard
// (RB-010-13, `analyzer`); the palette's popup (RB-060-11, `palette`), the plugin panels
// (RB-060-12, `panels`, with the findings since RB-080-69) and the status bar's message
// history (RB-080-15, `status-history`) load the same pages in windows of their own; the Portal
// panel (RB-080-05, `portal`) and the setup wizard (RB-091-05, `setup`, `setup/project`) open in
// the pages window; the sitemap gallery (RB-090-86, `sitemap-gallery?run=<id>`) loads them in its
// own window; the bug report's preview (RB-093-04, `bug-report`) opens in the pages window. An
// unknown route shows the settings. Each page loads when its route is first shown (RB-090-38).

import { defineAsyncComponent, type Component } from 'vue';
import { GALLERY_ROUTE } from '../sitemap/gallery/gallery';

/** A page's module, loaded on demand. */
export type PageLoader = () => Promise<{ default: Component }>;

const settings: PageLoader = () => import('./SettingsPage.vue');

/** The pages by route, each its own chunk (RB-090-38): the palette's popup and the panels window
 * no longer load the settings page with them. */
export const PAGES: Record<string, PageLoader> = {
  settings,
  trust: () => import('../trust/TrustPage.vue'),
  analyzer: () => import('../analyzer/AnalyzerWizard.vue'),
  palette: () => import('../palette/PalettePage.vue'),
  panels: () => import('../panels/PanelsPage.vue'),
  'status-history': () => import('../statusbar/StatusHistory.vue'),
  portal: () => import('../portal/PortalPage.vue'),
  setup: () => import('../firstrun/SetupWizard.vue'),
  [GALLERY_ROUTE]: () => import('../sitemap/gallery/SitemapGalleryPage.vue'),
  'bug-report': () => import('../bugreport/BugReportPreview.vue'),
};

const pages = new Map<string, Component>();

/** The page of `route`, the settings for an unknown one: loaded when first shown, the same
 * component every time after. */
export function pageOf(route: string): Component {
  const known = Object.hasOwn(PAGES, route) ? route : 'settings';
  let page = pages.get(known);
  if (page === undefined) {
    page = defineAsyncComponent(PAGES[known] ?? settings);
    pages.set(known, page);
  }
  return page;
}
