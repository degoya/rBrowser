// The findings of a check run as the shell holds them (RB-060-01). The run's types live with the
// IPC protocol (`../ipc/findings.ts`, the mirror of `rb_plugin_types::FindingsFile`); the
// messages the panel sends and receives are those of `../ipc/protocol.ts`. The panel talks
// through a `FindingsChannel`, which `FindingsPage.vue` builds on the window's IPC.

import type { CoreToShell, ShellToCore } from '../ipc/protocol';

export type {
  CheckFailure,
  CheckJob,
  CheckJobState,
  ChecksProgress,
  CheckTarget,
  CheckedView,
  ElementRef,
  FindingMessage,
  FindingsRun,
  FindingsSummary,
  MergedFinding,
  Severity,
} from '../ipc/findings';

/** Shell -> core: run the checks, mark a finding's element, open the rule settings, run a
 * command (`checks.cancel`, RB-080-89). */
export type FindingsRequest = Extract<
  ShellToCore,
  { type: 'runChecks' } | { type: 'highlightFinding' } | { type: 'openSettings' } | { type: 'runCommand' }
>;

/** Core -> shell: a finished run with the texts of the plugin keys its findings name (resolved
 * by the core from the plugins' locales). */
export type FindingsEvent = Extract<CoreToShell, { type: 'findings' }>;

/** Core -> shell: a check run's progress, while it goes and when it ends (RB-080-89). */
export type ProgressEvent = Extract<CoreToShell, { type: 'checksProgress' }>;

/** What the panel needs of the IPC client. */
export interface FindingsChannel {
  request(message: FindingsRequest): Promise<unknown>;
  on(listener: (event: FindingsEvent | ProgressEvent) => void): () => void;
  /** The last run, `null` before the first one (the window opened after it). */
  last(): Promise<FindingsEvent | null>;
  /** The progress of the run in progress or the last one; `null` before the first. */
  progress(): Promise<ProgressEvent | null>;
}
