// The findings of a check run (RB-060-01) as the event `findings` carries them: the TypeScript
// mirror of `rb_plugin_types::FindingsFile` (`schema/findings.v1.json`). On the Rust side the
// run is plain JSON in `crates/rb-ipc` (no plugin crate there); `protocol.ts` embeds it.

/** A view id, as in `protocol.ts`. */
type ViewId = number;

export type Severity = 'info' | 'warning' | 'error';

/** `"all"` or a view id, as in every MCP tool. */
export type CheckTarget = 'all' | ViewId;

export interface FindingMessage {
  /** An i18n key of the reporting plugin's locales. */
  key: string;
  params?: Record<string, string>;
}

export interface ElementRef {
  selector?: string;
  path?: string;
}

export interface MergedFinding {
  /** Stable across runs: rule (or shared id), URL and element. */
  id: string;
  ruleId: string;
  plugins: string[];
  severity: Severity;
  message: FindingMessage;
  views: ViewId[];
  breakpoints: string[];
  url?: string;
  element?: ElementRef;
  helpUrl?: string;
  data?: unknown;
  sharedId?: string;
}

export interface FindingsSummary {
  total: number;
  error: number;
  warning: number;
  info: number;
  byPlugin: Record<string, number>;
}

export interface CheckedView {
  id: ViewId;
  breakpoint?: string;
}

export interface CheckFailure {
  plugin: string;
  view?: ViewId;
  code?: string;
  /** The code's parameters (`{service}`, RB-080-98). */
  params?: Record<string, string>;
}

/** A plugin that did not check a view (`reason` `engine`): its findings there are unknown, not
 * clean (RB-080-98, `rb_plugin_types::CheckSkip`). */
export interface CheckSkip {
  plugin: string;
  view?: ViewId;
  reason: string;
  engine?: string;
}

export interface FindingsRun {
  version: number;
  runId: string;
  startedAt: number;
  target: CheckTarget;
  url?: string;
  plugins: string[];
  views: CheckedView[];
  findings: MergedFinding[];
  summary: FindingsSummary;
  failures?: CheckFailure[];
  skipped?: CheckSkip[];
}

/** One `checks.ignore` entry: globs of the rule id, page URL and element selector. */
export interface IgnoreEntry {
  rule?: string;
  url?: string;
  selector?: string;
}

/** What the project's `checks` settings turned off over the user's (RB-060-01). */
export interface ProjectOff {
  rules: string[];
  ignore: IgnoreEntry[];
}

/** Where one plugin x view job of a check run is (RB-080-89, `rb_ipc::CheckJobState`). */
export type CheckJobState = 'queued' | 'running' | 'done' | 'failed' | 'skipped';

/** One plugin over one view; no `view`: the plugin as a whole, or a run without views. */
export interface CheckJob {
  plugin: string;
  view?: ViewId;
  breakpoint?: string;
  state: CheckJobState;
  /** The findings it kept (`done`). */
  findings?: number;
  /** The failure's i18n key (`failed`) or the skip reason's (`skipped`). */
  reason?: string;
  /** The reason key's parameters (`{service}`, `{engine}`, RB-080-98). */
  reasonParams?: Record<string, string>;
  elapsedMs?: number;
}

/** A check run's progress, pushed as `checksProgress` while it goes and when it ends. */
export interface ChecksProgress {
  runId: string;
  /** How long the run has gone, as of this message. */
  elapsedMs: number;
  /** How long the whole run takes, from the last run's timings; absent without one. */
  estimateMs?: number;
  jobs: CheckJob[];
  finished: boolean;
  cancelled?: boolean;
}
