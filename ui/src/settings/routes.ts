// The pages of the pages window by route (`#/<page>`). Each page gets the IPC client and the
// route's key: the settings, the trust dialog (RB-010-09, `trust`) and the analyser's wizard
// (RB-010-13, `analyzer`); the palette's popup (RB-060-11, `palette`), the plugin panels
// (RB-060-12, `panels`, with the findings since RB-080-69) and the status bar's message
// history (RB-080-15, `status-history`) load the same pages in windows of their own; the Portal
// panel (RB-080-05, `portal`) opens in the pages window. An unknown route shows the settings.

import type { Component } from 'vue';
import AnalyzerWizard from '../analyzer/AnalyzerWizard.vue';
import PalettePage from '../palette/PalettePage.vue';
import PanelsPage from '../panels/PanelsPage.vue';
import StatusHistory from '../statusbar/StatusHistory.vue';
import PortalPage from '../portal/PortalPage.vue';
import TrustPage from '../trust/TrustPage.vue';
import SettingsPage from './SettingsPage.vue';

export const PAGES: Record<string, Component> = {
  settings: SettingsPage,
  trust: TrustPage,
  analyzer: AnalyzerWizard,
  palette: PalettePage,
  panels: PanelsPage,
  'status-history': StatusHistory,
  portal: PortalPage,
};

/** The page of `route`, the settings for an unknown one. */
export function pageOf(route: string): Component {
  return PAGES[route] ?? SettingsPage;
}
