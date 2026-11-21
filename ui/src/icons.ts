// Every icon the shell uses. The list is bundled into the build (`vite.config.ts`,
// `icon.clientBundle`), so the shell never asks the Iconify API over the network.

import { ACTIVITY_ICONS } from './activity/icons';
import { FINDINGS_ICONS } from './findings/icons';
import { PANEL_BUNDLED_ICONS } from './panels/icons';
import { PRESET_ICONS } from './presets/icons';
import { RECORDER_ICONS } from './recorder/icons';
import { STATUS_ICONS } from './statusbar/icons';
import { GALLERY_ICONS } from './sitemap/gallery/icons';
import { VIDEO_ICONS } from './videos/icons';

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
  // Resume a paused recording (RB-090-60).
  recordResume: 'i-lucide-play',
  bookmarkAdd: 'i-lucide-bookmark-plus',
  // Add a view (RB-080-41).
  addView: 'i-lucide-plus',
  // The toolbar's overflow menu and the selects shrunk to an icon (RB-080-52).
  more: 'i-lucide-ellipsis',
  // The chevron after a toolbar pulldown's value (RB-090-45, `toolbar/PulldownButton.vue`).
  pulldown: 'i-lucide-chevron-down',
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
  // The portal's button in the toolbar (RB-090-62): pairing a real phone or tablet.
  portal: 'i-lucide-tablet-smartphone',
  // The toolbar's split buttons and the main menu (RB-090-63).
  devtools: 'i-lucide-square-code',
  screenshot: 'i-lucide-camera',
  video: 'i-lucide-video',
  videoStop: 'i-lucide-square',
  checks: 'i-lucide-list-checks',
  menu: 'i-lucide-menu',
  ...PRESET_ICONS,
  ...STATUS_ICONS,
  // A check run's job states in the findings panel (RB-080-89).
  ...FINDINGS_ICONS,
  // The Videos panel (RB-110-12).
  ...VIDEO_ICONS,
  // The sitemap gallery page (RB-090-86).
  ...GALLERY_ICONS,
  // The Recorder and Flows panels (RB-090-60).
  ...RECORDER_ICONS,
  // The activity bar (RB-090-67).
  ...ACTIVITY_ICONS,
} as const;

/** The `collection:name` form `clientBundle.icons` takes. */
export const BUNDLED_ICONS: string[] = [
  ...Object.values(ICONS).map((name) => name.replace(/^i-lucide-/, 'lucide:')),
  // The plugin panels page's icons (RB-080-61): the bundled plugins' panel icons and the dock's.
  ...PANEL_BUNDLED_ICONS,
];
