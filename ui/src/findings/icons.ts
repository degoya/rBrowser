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
  /** The Export button and its arrow (RB-092-05). */
  export: 'i-lucide-download',
  exportMenu: 'i-lucide-chevron-down',
  /** "Show at N px" of a width sweep finding (RB-093-01). */
  showWidth: 'i-lucide-move-horizontal',
  /** "Run <check> again" in the progress details and beside the plugin filter (RB-094-17). */
  rerun: 'i-lucide-rotate-cw',
  /** Visual regression's "Update baselines", the secondary action beside Compare (RB-099-10). */
  baselines: 'i-lucide-camera',
} as const;
