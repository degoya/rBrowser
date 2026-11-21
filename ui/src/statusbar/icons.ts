// The icons of the status bar and its history (RB-080-15). `src/icons.ts` bundles them
// (`BUNDLED_ICONS`), so the shell never asks the Iconify API over the network.

export const STATUS_ICONS = {
  statusError: 'i-lucide-circle-x',
  statusWarning: 'i-lucide-triangle-alert',
  statusInfo: 'i-lucide-info',
  statusSuccess: 'i-lucide-circle-check',
  statusClose: 'i-lucide-x',
  statusHistory: 'i-lucide-history',
  statusCopy: 'i-lucide-copy',
  // A saved bug report's Markdown is on the clipboard (RB-093-04, `StatusCopyButton.vue`).
  statusCopied: 'i-lucide-check',
  // The canvas zoom's buttons (RB-080-94).
  canvasZoomOut: 'i-lucide-minus',
  canvasZoomIn: 'i-lucide-plus',
  // The level button: its presets open as a native menu above it (RB-090-18).
  canvasZoomPresets: 'i-lucide-chevron-up',
} as const;
