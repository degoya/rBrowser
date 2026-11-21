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
  /** The views' clock is not the real time (RB-094-07). */
  activityClock: 'i-lucide-clock',
  /** The animations do not run at 100 % (RB-094-07). */
  activityAnimations: 'i-lucide-gauge',
  /** Move stopped animations a step on (`overlays.animations`). */
  activityStep: 'i-lucide-step-forward',
  /** Every view's animations at 100 % again. */
  activityNormal: 'i-lucide-play',
  /** The real time in every view again (`emulation.clock`). */
  activityRealTime: 'i-lucide-timer-reset',
  /** Fold an entry into its toolbar indicator. */
  activityClose: 'i-lucide-x',
  /** Show a folded entry in the bar again. */
  activityUnfold: 'i-lucide-panel-top-open',
  /** A plugin call that reports its progress (RB-093-01). */
  activityPlugin: 'i-lucide-puzzle',
  /** The presentation over the internet is live (RB-096-05). */
  activityPresent: 'i-lucide-radio-tower',
  /** Copy the presentation's one-time link. */
  activityCopyLink: 'i-lucide-copy',
  /** A new one-time link (`portal.present.new_link`). */
  activityNewLink: 'i-lucide-link-2',
  /** End the presentation (`portal.present.stop`). */
  activityEndPresent: 'i-lucide-square',
  /** Pseudo-class states forced on an element (RB-094-16). */
  activityForced: 'i-lucide-mouse-pointer-click',
  /** Take every forced state back (`view.force_state.reset`). */
  activityResetStates: 'i-lucide-rotate-ccw',
} as const;
