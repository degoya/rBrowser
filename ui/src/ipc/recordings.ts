// The Recorder and Flows panels' messages (RB-090-60), mirror of rb-ipc's `recordings.rs`; kept
// beside `protocol.ts`, which re-exports it. `getRecordings` and `saveRecording` are answered
// with `recordings`, `getRecording` with `recording`, `exportRecording` with `recordingExport`
// (a preview when `write` is false) and `getFlowRun` with `flowRun`; `recordingsChanged` and
// `flowRunChanged` tell the panels to ask again.

import type { Recording } from '../recorder/recording';

/** One stored recording. */
export interface RecordingEntry {
  /** Its name, also its file name. */
  name: string;
  /** Its steps; 0 when the file could not be read. */
  steps: number;
  /** When the file was last written, ms since the epoch. */
  modifiedMs: number;
}

/** The stored recordings of the open project, newest first. */
export interface RecordingList {
  project: boolean;
  /** The recordings' folder, relative to the project root. */
  folder?: string;
  /** Why the folder could not be read. */
  error?: string;
  recordings: RecordingEntry[];
  /** The recording running now. */
  recordingNow?: string;
}

/** What an export makes. */
export type RecordingExportFormat = 'flow' | 'spec' | 'both';

/** One file of an export: its path once written, else its name. */
export interface RecordingExportFile {
  path: string;
  content: string;
}

/** An export's files. */
export interface RecordingExport {
  written: boolean;
  /** Where they were written, relative to the project root. */
  folder?: string;
  files: RecordingExportFile[];
  /** A preview with a flow: where "Save" puts it (`.rbrowser/flows/<name>.ts`, RB-090-66). */
  flowFile?: string;
  /** A flow of that name exists: "Save" replaces it only with `overwrite`. */
  flowExists?: boolean;
}

/** One view's status in a run going on, as the runner reports it. */
export interface FlowViewStatus {
  view: number;
  status: string;
}

/** A run going on. */
export interface FlowRunning {
  runId: string;
  flow: string;
  /** Every view of the run `queued` and every view left out `skipped` at the start, then as reported. */
  views: FlowViewStatus[];
  /** When it started, ms since the epoch. */
  startedAtMs: number;
  /** The flows of "Run all" still waiting after this one. */
  queued?: string[];
}

/** A view's outcome in `result.json` (`crates/rbrowser/src/flows/result.rs`). */
export type FlowViewOutcome = 'passed' | 'failed' | 'skipped' | 'timedOut';

/** One view of a finished run. */
export interface FlowViewResult {
  viewId: number;
  label: string;
  breakpoint?: string | null;
  engine: string;
  status: FlowViewOutcome;
  durationMs: number;
  steps: { name: string; status: string; durationMs: number }[];
  error?: {
    message: string;
    stack?: string;
    step?: string | null;
    line?: number | null;
    column?: number | null;
    file?: string;
  } | null;
  /** The screenshot's file name inside the run's folder. */
  screenshot?: string | null;
  /** Why it was skipped. */
  reason?: string;
}

/** A finished run, its `result.json`. */
export interface FlowRunResult {
  version: number;
  runId: string;
  flow: string;
  file: string;
  startedAt: string;
  durationMs: number;
  passed: boolean;
  views: FlowViewResult[];
  error?: string;
}

/** The run going on and the last one's result. */
export interface FlowRunState {
  project: boolean;
  running?: FlowRunning;
  last?: FlowRunResult;
}

/** The panels' requests. */
export type RecordingRequest =
  | { type: 'getRecordings' }
  | { type: 'getRecording'; name: string }
  /** The edited recording, written over its file; only the shell sends this. */
  | { type: 'saveRecording'; name: string; recording: Recording }
  | {
      type: 'exportRecording';
      name: string;
      format: RecordingExportFormat;
      exportName?: string;
      /** Write the files; a preview without. */
      write: boolean;
      /** Replace a flow of the same name: the person said so in the dialog. */
      overwrite?: boolean;
    }
  | { type: 'getFlowRun' };

/** The answers and events. */
export type RecordingEvent =
  | ({ type: 'recordings' } & RecordingList)
  | { type: 'recording'; name: string; recording: Recording }
  | ({ type: 'recordingExport' } & RecordingExport)
  | { type: 'recordingsChanged' }
  | ({ type: 'flowRun' } & FlowRunState)
  | { type: 'flowRunChanged' };
