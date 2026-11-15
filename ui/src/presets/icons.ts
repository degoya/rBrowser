// The icons of the layout and environment controls (RB-010-12). `src/icons.ts` bundles them
// (`BUNDLED_ICONS`), so the shell never asks the Iconify API over the network.

export const PRESET_ICONS = {
  // One window: not the four corners of zoom to fit (owner, build 22481e2).
  focusMode: 'i-lucide-app-window',
  focusPrev: 'i-lucide-chevron-left',
  focusNext: 'i-lucide-chevron-right',
  zoomToFit: 'i-lucide-maximize',
  // The layout modes (RB-080-04).
  modeGrid: 'i-lucide-layout-grid',
  modeHorizontal: 'i-lucide-columns-3',
  modeSplit: 'i-lucide-panel-left',
  modeCanvas: 'i-lucide-frame',
  splitSwap: 'i-lucide-arrow-left-right',
  minimap: 'i-lucide-map',
  // Save layout as preset: not the bookmark-plus of "Bookmark this page" beside it.
  savePreset: 'i-lucide-save',
} as const;
