// The toolbar's groups (RB-080-52): what always stays, what shrinks to an icon and in which order
// the rest leaves the row for the overflow menu. `ShellToolbar.vue` wraps each group in a
// `ToolbarGroup` with one of these ids.

/** Always in the row: the logo (it alone gives way below the smallest sensible width) ... */
export const LOGO = 'logo';
/** ... and back, forward, reload. The address bar is no group: it takes what is left. */
export const NAVIGATION = 'navigation';

/** The groups before the address bar, which stay in the first row when the toolbar wraps. */
export const FIRST_ROW: readonly string[] = ['minimap-start'];

/**
 * The order in which the groups leave the row for the main menu `≡`, the first one first
 * (RB-090-63, the menu audit): the canvas minimap, the pulldowns that have their rows in the
 * menu as well, the checks and the emulation, at the end what is used most or tells the state of
 * the window (DevTools, sync, the trust badge), the settings and the plugin panels' switch.
 */
export const OVERFLOW_ORDER: readonly string[] = [
  'minimap-end',
  'minimap-start',
  'bookmarks',
  'environment',
  'checks',
  'emulation',
  'frameworks',
  'layout',
  'sitemap',
  'capture',
  'recorder',
  // The portal's button (RB-090-62): while it runs it says so, so it stays long.
  'portal',
  'browsersync',
  'devtools',
  'sync',
  'settings',
  'trust',
  // The plugin panels' switch at the right end leaves last (RB-080-79).
  'panels',
];

/** The groups whose pulldown shrinks to an icon with a tooltip before anything leaves the row. */
export const COMPACT_ORDER: readonly string[] = ['bookmarks', 'environment', 'layout'];

/**
 * The groups that are there only while they tell something - a recording, a sitemap run, sync
 * paused by Browsersync, a framework's DevTools on offer, a restricted workspace:
 * hidden, they put a mark on the overflow menu's button.
 */
export const NOTICE_GROUPS: readonly string[] = ['browsersync', 'frameworks', 'trust', 'recorder', 'sitemap'];

/** The command that opens the main menu `≡` (`Leader P T`); the core sends it as `uiCommand`. */
export const OPEN_COMMAND = 'ui.toolbar.overflow.open';
/** The setting: `menu` or `wrap`. */
export const OVERFLOW_SETTING = 'ui.toolbar.overflow';
