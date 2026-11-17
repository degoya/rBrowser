// The Videos panel's icons (RB-110-12), Lucide; `src/icons.ts` bundles them (`BUNDLED_ICONS`).

export const VIDEO_ICONS = {
  /** The panel's entry in the side bar. */
  videos: 'i-lucide-film',
  play: 'i-lucide-play',
  pause: 'i-lucide-pause',
  reveal: 'i-lucide-folder-open',
  copyPath: 'i-lucide-clipboard-copy',
  remove: 'i-lucide-trash',
  /** Export a WebM as MP4 to share it (RB-090-52). */
  exportMp4: 'i-lucide-share-2',
  /** Convert an old MP4 to WebM, which plays here. */
  convertWebm: 'i-lucide-refresh-cw',
  /** An export or conversion waits or runs. */
  converting: 'i-lucide-loader-circle',
} as const;
