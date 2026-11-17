// The icons of the plugin panels page (RB-080-61): the side bar's panel icons and the header's.
// A manifest names a panel's icon (`icon`, a Lucide name the plugin host checks); the shell draws
// only the names listed here - bundled into the build through `src/icons.ts` (`BUNDLED_ICONS`,
// `icon.clientBundle`), so nothing is fetched over the network - and the fallback for any other.
// A test holds every bundled plugin's icon to this list.

/** The icons the bundled plugins' panels name, sorted. */
export const PANEL_ICON_NAMES: readonly string[] = [
  'atom',
  'bug',
  'camera',
  'chart-pie',
  'code',
  'component',
  'database',
  'gauge',
  'glasses',
  'hand',
  'image',
  'layers',
  'list-todo',
  'list-tree',
  'network',
  'pipette',
  'ruler',
  'search-check',
  'smartphone',
  'terminal',
  'text-cursor-input',
  'type',
];

/** A panel whose manifest names no icon, or one the shell does not bundle. */
export const FALLBACK_PANEL_ICON = 'puzzle';

/** The header's and the side bar's own icons. */
export const DOCK_ICONS = {
  dock: 'i-lucide-layout-panel-left',
  close: 'i-lucide-x',
  /** The Dock menu's places (RB-080-93), the button shows the current one's. */
  position: {
    left: 'i-lucide-panel-left',
    bottom: 'i-lucide-panel-bottom',
    right: 'i-lucide-panel-right',
    floating: 'i-lucide-app-window',
  },
  /** The Dock menu's "Hide the panels" (RB-080-93). */
  hide: 'i-lucide-eye-off',
  /** Nuxt UI's tick of a checked menu entry (`appConfig.ui.icons.check`), bundled here. */
  check: 'i-lucide-check',
  /** The Dock menu button's trailing chevron. */
  menu: 'i-lucide-chevron-down',
  collapse: {
    left: 'i-lucide-panel-left-close',
    right: 'i-lucide-panel-right-close',
    bottom: 'i-lucide-panel-bottom-close',
  },
  expand: {
    left: 'i-lucide-panel-left-open',
    right: 'i-lucide-panel-right-open',
    bottom: 'i-lucide-panel-bottom-open',
  },
  sidebarCollapse: 'i-lucide-chevrons-left',
  sidebarExpand: 'i-lucide-chevrons-right',
  /** The core's Findings entry (RB-080-69). */
  findings: 'i-lucide-list-checks',
} as const;

const BUNDLED = new Set(PANEL_ICON_NAMES);

/** The icon class for a manifest's `icon`: a bundled name, else the fallback - never a URL or
 * markup, whatever the core sent. */
export function panelIcon(name: string | null | undefined): string {
  return `i-lucide-${typeof name === 'string' && BUNDLED.has(name) ? name : FALLBACK_PANEL_ICON}`;
}

/** Every icon of the page in the `collection:name` form `clientBundle.icons` takes. */
export const PANEL_BUNDLED_ICONS: string[] = [
  ...PANEL_ICON_NAMES.map((name) => `lucide:${name}`),
  `lucide:${FALLBACK_PANEL_ICON}`,
  ...[
    DOCK_ICONS.dock,
    DOCK_ICONS.close,
    DOCK_ICONS.sidebarCollapse,
    DOCK_ICONS.sidebarExpand,
    DOCK_ICONS.findings,
    DOCK_ICONS.hide,
    DOCK_ICONS.check,
    DOCK_ICONS.menu,
    ...Object.values(DOCK_ICONS.position),
    ...Object.values(DOCK_ICONS.collapse),
    ...Object.values(DOCK_ICONS.expand),
  ].map((name: string) => name.replace(/^i-lucide-/, 'lucide:')),
];
