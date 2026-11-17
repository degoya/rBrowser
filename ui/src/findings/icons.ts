// The icons of a check run's progress in the findings panel (RB-080-89), one per job state, so
// the state is never told by colour alone. `src/icons.ts` bundles them (`BUNDLED_ICONS`).

export const FINDINGS_ICONS = {
  checkQueued: 'i-lucide-clock',
  checkRunning: 'i-lucide-loader-circle',
  checkDone: 'i-lucide-circle-check',
  checkFailed: 'i-lucide-circle-x',
  checkSkipped: 'i-lucide-circle-slash',
  /** Opens a finished run's progress per plugin (RB-080-98). */
  checkDetails: 'i-lucide-chevron-down',
} as const;
