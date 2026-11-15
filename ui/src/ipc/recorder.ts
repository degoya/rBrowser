// The recorder's part of the snapshot (RB-050-01), mirror of rb-ipc's `RecorderStatus`; kept
// beside `protocol.ts`, which re-exports it.

/** Whether a recording takes steps. */
export type RecorderPhase = 'recording' | 'paused';

/** A running recording: the toolbar's indicator. */
export interface RecorderStatus {
  phase: RecorderPhase;
  /** The recording's name, also its file name. */
  name: string;
  /** The steps so far. */
  steps: number;
}
