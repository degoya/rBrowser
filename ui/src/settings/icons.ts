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
  // Check the plugin catalogue for updates (RB-091-01); the CDP proxy's new token (RB-090-63).
  refresh: 'i-lucide-refresh-cw',
  // The commands beside the CDP proxy and MCP switches (RB-090-63).
  endpoint: 'i-lucide-link',
  script: 'i-lucide-code',
  terminal: 'i-lucide-terminal',
  // "Install and restart" under the self-update switch (RB-090-14).
  install: 'i-lucide-circle-arrow-down',
  // The setup wizard (RB-091-05): its command, its steps, the plugin step without a network.
  wizard: 'i-lucide-wand-sparkles',
  welcome: 'i-lucide-sparkles',
  plugins: 'i-lucide-puzzle',
  project: 'i-lucide-folder-cog',
  basics: 'i-lucide-sliders-horizontal',
  done: 'i-lucide-circle-check',
  // The wizard's AI agent step and the MCP setup (RB-091-09).
  agent: 'i-lucide-bot',
  offline: 'i-lucide-wifi-off',
  next: 'i-lucide-arrow-right',
  previous: 'i-lucide-arrow-left',
} as const;

/** The `collection:name` form `clientBundle.icons` takes. */
export const SETTINGS_BUNDLED_ICONS: string[] = Object.values(SETTINGS_ICONS).map((name) =>
  name.replace(/^i-lucide-/, 'lucide:'),
);
