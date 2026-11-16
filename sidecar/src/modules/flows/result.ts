// The shapes of a flow run (RB-040-07): what rBrowser sends (`RunJob`), what each view reports
// (`ViewResult`), and where an error happened in the flow file (`locate`). The Rust side
// (crates/rbrowser/src/flows/result.rs) reads these fields by name.

/** How long a screenshot after a failure may take. */
export const SCREENSHOT_TIMEOUT_MS = 5000;
/** How long letting go of a view's connection may take. */
export const CLOSE_TIMEOUT_MS = 5000;
/**
 * What a view may take past its timeout: the failure screenshot and the close. The run's budget
 * (`spawn.ts`, and `VIEW_OVERHEAD` in crates/rbrowser/src/flows/run.rs) adds it per round.
 */
export const VIEW_OVERHEAD_MS = SCREENSHOT_TIMEOUT_MS + CLOSE_TIMEOUT_MS;

/** The status of one view; `skipped` is decided by rBrowser (Firefox views, no page). */
export type ViewStatus = 'passed' | 'failed' | 'skipped' | 'timedOut';

/** The `view` global of a flow. */
export interface FlowView {
  id: number;
  label: string;
  width: number;
  height: number;
  engine: string;
  breakpoint: string | null;
}

/** How the worker reaches one view: a CDP endpoint that shows only that view's page. */
export interface Endpoint {
  url: string;
  headers: Record<string, string>;
}

export interface JobView extends FlowView {
  endpoint: Endpoint;
}

/** The flow itself: rBrowser reads the file (after the trust check) and sends its text. */
export interface FlowSource {
  name: string;
  /** TypeScript; types are stripped, never checked. */
  source: string;
  /** The path errors name: the Linux path for a WSL project. */
  file: string;
}

/** One run as the worker gets it on stdin. */
export interface RunJob {
  runId: string;
  flow: FlowSource;
  views: JobView[];
  /** The URL of the active environment, the `env` global. */
  env: string | null;
  /** Per view. */
  timeoutMs: number;
  /** Views at once; 0 runs all at once. */
  parallel: number;
  screenshotOnFailure: boolean;
  /** `.rbrowser/out/flows/<runId>`; the screenshots go here. */
  outDir: string;
}

export interface StepResult {
  name: string;
  status: 'passed' | 'failed';
  durationMs: number;
}

export interface ErrorInfo {
  message: string;
  stack: string;
  /** The step that was running, if any. */
  step: string | null;
  /** In the flow file, 1-based. */
  line: number | null;
  column: number | null;
  file: string;
}

export interface ViewResult {
  viewId: number;
  label: string;
  breakpoint: string | null;
  engine: string;
  status: ViewStatus;
  durationMs: number;
  steps: StepResult[];
  error: ErrorInfo | null;
  /** The screenshot's file name inside the run's folder. */
  screenshot: string | null;
}

/** Line and column of the first frame of `stack` in `file`. */
export function locate(stack: string, file: string): { line: number; column: number } | null {
  const marker = `${file}:`;
  for (const frame of stack.split('\n')) {
    const at = frame.indexOf(marker);
    if (at < 0) {
      continue;
    }
    const match = /^(\d+)(?::(\d+))?/.exec(frame.slice(at + marker.length));
    if (match?.[1] !== undefined) {
      return { line: Number(match[1]), column: Number(match[2] ?? 1) };
    }
  }
  return null;
}

/** `text` with every secret replaced (the endpoint's token never reaches a result). */
export function redact(text: string, secrets: readonly string[]): string {
  return secrets.filter((s) => s.length >= 8).reduce((out, secret) => out.split(secret).join('<redacted>'), text);
}

/** The secrets of a job: every header value of every endpoint. */
export function secretsOf(job: Pick<RunJob, 'views'>): string[] {
  return job.views.flatMap((view) =>
    Object.values(view.endpoint.headers).flatMap((value) => [value, value.replace(/^Bearer\s+/i, '')]),
  );
}

/** A file name for a view's screenshot: its label made safe, and its id. */
export function screenshotName(view: Pick<FlowView, 'id' | 'label'>): string {
  const label = view.label.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40);
  return `${label === '' ? 'view' : label}-${view.id}.png`;
}
