// The Recorder and Flows panels' messages (RB-090-60), mirror of rb-ipc's `recordings.rs`; kept
// beside `protocol.ts`, which re-exports it. `getRecordings` and `saveRecording` are answered
// with `recordings`, `getRecording` with `recording`, `exportRecording` with `recordingExport`
// (a preview when `write` is false) and `getFlowRun` with `flowRun`; `recordingsChanged` and
// `flowRunChanged` tell the panels to ask again; `flowStep` pushes each step of a run (RB-090-84);
// `recordingSteps` the steps of the recording that runs (RB-090-89).

import type { Locator, Recording, Step } from '../recorder/recording';

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
  /** The step it is at (RB-090-68); absent before its first. */
  step?: FlowStepAt;
}

/** The step a view of a run going on is at (RB-090-68). */
export interface FlowStepAt {
  /** The steps the view started so far, this one included: the n of "step n/m". */
  number: number;
  name: string;
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
  /** The `step(` calls of the flow's text (RB-090-68): the m of "step n/m". */
  totalSteps?: number;
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
    /** The step's id in the recording, for a name `Step <id>: ...` (RB-090-84). */
    stepId?: number | null;
    /** The message's first line, masked: what the Recorder panel shows. */
    summary?: string | null;
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
  /** The breakpoints the flow names that no view showed, so the run skipped them (RB-090-84). */
  skippedBreakpoints?: string[];
  error?: string;
}

/** The pause before a step at the pace "As recorded", as the exporter writes it (RB-090-84). */
export interface StepPause {
  step: number;
  ms: number;
}

/** A step of the flow run going on started or ended (RB-090-84). */
export interface FlowStep {
  runId: string;
  /** The flow; a played recording's name. */
  flow: string;
  view: number;
  /** The step's name (`Step 3: click` in a played recording). */
  name: string;
  /** The recording's step id, for a name `Step <id>: ...`. */
  stepId?: number;
  state: 'running' | 'passed' | 'failed';
  /** Why a failed step failed: its error's first line, masked. */
  message?: string;
  /** A step that starts: the steps its view started so far, this one included (RB-090-68). */
  number?: number;
  /** The `step(` calls of the flow's text, as `FlowRunning.totalSteps`. */
  total?: number;
}

/** Whether the live list goes on: it records, it was saved, or `showLiveSteps` went off. */
export type LiveState = 'recording' | 'ended' | 'hidden';

/**
 * The steps of the recording that runs (RB-090-89): pushed while it records, with
 * `automation.recorder.showLiveSteps` - only what changed, the steps from `from` on - and all of
 * them after `resendRecordingSteps`. `state` says when the list ends and why.
 */
export interface RecordingSteps {
  name: string;
  from: number;
  steps: Step[];
  state: LiveState;
  /** While a recording continues from one of its steps (RB-092-01): the new steps land after it. */
  continueAfter?: number;
}

/**
 * The element "Pick in view" picked for the Recorder panel's editor (RB-092-01): its locator,
 * its visible text (never a field's value), the breakpoints whose views have it (empty: all) and
 * a `<select>`'s options.
 */
export interface PickedElement {
  view: number;
  locator: Locator;
  text: string;
  onlyIn: string[];
  options?: { value: string; label: string }[];
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
  | { type: 'getFlowRun' }
  /** Push every step of the recording that runs again (a panel catching up); answers `ack`. */
  | { type: 'resendRecordingSteps' };

/** The answers and events. */
export type RecordingEvent =
  | ({ type: 'recordings' } & RecordingList)
  | { type: 'recording'; name: string; recording: Recording; pauses?: StepPause[] }
  | ({ type: 'recordingExport' } & RecordingExport)
  | { type: 'recordingsChanged' }
  | ({ type: 'flowRun' } & FlowRunState)
  | { type: 'flowRunChanged' }
  | ({ type: 'flowStep' } & FlowStep)
  | ({ type: 'recordingSteps' } & RecordingSteps)
  | ({ type: 'recorderElementPicked' } & PickedElement)
  /** A recording editor's command without its step (RB-092-01): run it on the step in focus. */
  | { type: 'recorderStepCommand'; command: string };
