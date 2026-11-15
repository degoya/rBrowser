// Every icon the shell uses. The list is bundled into the build (`vite.config.ts`,
// `icon.clientBundle`), so the shell never asks the Iconify API over the network.

import { FINDINGS_ICONS } from './findings/icons';
import { PANEL_BUNDLED_ICONS } from './panels/icons';
import { PRESET_ICONS } from './presets/icons';
import { STATUS_ICONS } from './statusbar/icons';

export const ICONS = {
  back: 'i-lucide-arrow-left',
  forward: 'i-lucide-arrow-right',
  reload: 'i-lucide-rotate-cw',
  settings: 'i-lucide-settings',
  analyze: 'i-lucide-scan-search',
  sync: 'i-lucide-link',
  layout: 'i-lucide-layout-grid',
  environment: 'i-lucide-server',
  url: 'i-lucide-globe',
  record: 'i-lucide-circle-dot',
  recordPaused: 'i-lucide-pause',
  recordStop: 'i-lucide-square',
  bookmarkAdd: 'i-lucide-bookmark-plus',
  // Add a view (RB-080-41).
  addView: 'i-lucide-plus',
  // The toolbar's overflow menu and the selects shrunk to an icon (RB-080-52).
  more: 'i-lucide-ellipsis',
  layoutPreset: 'i-lucide-layout-template',
  bookmarks: 'i-lucide-bookmark',
  // The breakpoint bar under every view (formerly the footer), its toolbar switch (RB-080-60).
  footer: 'i-lucide-panel-bottom',
  // rBrowser's own theme, chosen in the toolbar (RB-080-95, RB-090-10): a sun and a moon in one,
  // with a mark for the theme in effect - never the page emulation's separate sun and moon.
  theme: 'i-lucide-sun-moon',
  themeSystem: 'i-lucide-monitor',
  themeLight: 'i-lucide-sun-medium',
  themeDark: 'i-lucide-moon-star',
  ...PRESET_ICONS,
  ...STATUS_ICONS,
  // A check run's job states in the findings panel (RB-080-89).
  ...FINDINGS_ICONS,
} as const;

/** The `collection:name` form `clientBundle.icons` takes. */
export const BUNDLED_ICONS: string[] = [
  ...Object.values(ICONS).map((name) => name.replace(/^i-lucide-/, 'lucide:')),
  // The plugin panels page's icons (RB-080-61): the bundled plugins' panel icons and the dock's.
  ...PANEL_BUNDLED_ICONS,
];
