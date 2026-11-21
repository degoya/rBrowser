// The Recorder and Flows panels' icons (RB-090-60), Lucide; `src/icons.ts` bundles them
// (`BUNDLED_ICONS`). The keys carry the panel's name, so the spread into `ICONS` overrides no
// other set's key.

export const RECORDER_ICONS = {
  /** The Recorder panel's entry in the side bar. */
  recorderPanel: 'i-lucide-list-video',
  stepUp: 'i-lucide-arrow-up',
  stepDown: 'i-lucide-arrow-down',
  stepRemove: 'i-lucide-trash',
  recorderSave: 'i-lucide-save',
  recorderRevert: 'i-lucide-undo-2',
  recorderExport: 'i-lucide-file-code',
  recorderCopy: 'i-lucide-clipboard-copy',
  /** The Flows panel's entry in the side bar. */
  flowsPanel: 'i-lucide-workflow',
  flowRun: 'i-lucide-play',
  flowCancel: 'i-lucide-square',
  flowReveal: 'i-lucide-folder-open',
  flowPassed: 'i-lucide-circle-check',
  flowFailed: 'i-lucide-circle-x',
  flowSkipped: 'i-lucide-circle-minus',
  flowRunning: 'i-lucide-loader-circle',
  /** A view of a run going on that has not started yet (RB-090-71). */
  flowQueued: 'i-lucide-clock',
  /** "Delete" of a flow or a recording in its panel (RB-090-91). */
  entryDelete: 'i-lucide-trash',
  /** The recording editor (RB-092-01): add, try, edit, more, play from here, continue
   *  recording, duplicate, skip, pick in view. */
  stepAdd: 'i-lucide-plus',
  stepTry: 'i-lucide-play',
  stepEdit: 'i-lucide-pencil',
  stepMore: 'i-lucide-ellipsis',
  stepPlayFrom: 'i-lucide-fast-forward',
  stepContinue: 'i-lucide-circle-dot',
  stepDuplicate: 'i-lucide-copy',
  stepSkip: 'i-lucide-ban',
  stepPick: 'i-lucide-pipette',
} as const;
