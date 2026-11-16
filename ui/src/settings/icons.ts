// The icons of the pages window and the which-key bar, bundled like the toolbar's
// (`vite.config.ts` adds them to `icon.clientBundle`), so nothing is fetched over the network.

export const SETTINGS_ICONS = {
  search: 'i-lucide-search',
  reset: 'i-lucide-undo-2',
  add: 'i-lucide-plus',
  remove: 'i-lucide-trash-2',
  up: 'i-lucide-arrow-up',
  down: 'i-lucide-arrow-down',
  browse: 'i-lucide-file-search',
  browseFolder: 'i-lucide-folder-open',
  help: 'i-lucide-circle-help',
  copy: 'i-lucide-copy',
  json: 'i-lucide-braces',
  record: 'i-lucide-circle-dot',
  import: 'i-lucide-upload',
  export: 'i-lucide-download',
  close: 'i-lucide-x',
  keyboard: 'i-lucide-keyboard',
  duplicate: 'i-lucide-copy-plus',
  expand: 'i-lucide-chevron-right',
  // The back button of a tab row that scrolls (RB-080-81); forward is `expand`.
  back: 'i-lucide-chevron-left',
  grip: 'i-lucide-grip-vertical',
  // Nuxt UI's own icons of the searchable select (RB-080-72), bundled so nothing is fetched.
  chevronDown: 'i-lucide-chevron-down',
  check: 'i-lucide-check',
  loading: 'i-lucide-loader-circle',
  // The lead of a warning (`UAlert`, RB-080-88).
  warning: 'i-lucide-triangle-alert',
} as const;

/** The `collection:name` form `clientBundle.icons` takes. */
export const SETTINGS_BUNDLED_ICONS: string[] = Object.values(SETTINGS_ICONS).map((name) =>
  name.replace(/^i-lucide-/, 'lucide:'),
);
