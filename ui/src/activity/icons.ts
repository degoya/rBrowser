// The activity bar's own icons (RB-090-67), Lucide; `src/icons.ts` bundles them (`BUNDLED_ICONS`).
// An entry's activity keeps the icon it has elsewhere (the video, the recorder, the flows, the
// checks, the portal); these are the controls only the bar has. The keys carry the bar's name, so
// the spread into `ICONS` overrides no other set's key.

export const ACTIVITY_ICONS = {
  /** Views out of sync. */
  activityDesync: 'i-lucide-unlink',
  /** Load the active view's page in the views out of sync (`sync.resync`). */
  activityResync: 'i-lucide-refresh-cw',
  /** Add a check step for the element the next click picks (`recorder.check`). */
  activityCheck: 'i-lucide-square-check',
  /** Show a new pairing QR code (`portal.show_qr`). */
  activityQr: 'i-lucide-qr-code',
  /** Stop the portal; every device is disconnected. */
  activityDisconnect: 'i-lucide-unplug',
  /** Fold an entry into its toolbar indicator. */
  activityClose: 'i-lucide-x',
  /** Show a folded entry in the bar again. */
  activityUnfold: 'i-lucide-panel-top-open',
} as const;
