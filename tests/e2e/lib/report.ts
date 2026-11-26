// The report of an end-to-end run (RB-099-02): artifacts/e2e/report.json (per criterion its
// status, the measured values with their thresholds, the duration, the log) and report.md, the
// same as a table. Every text that goes into a log or the report passes `redact` first: the MCP
// and CDP tokens never leave the run (AGENTS.md).

export type Status = 'passed' | 'failed' | 'skipped';

/** One measured value of a criterion and the threshold it is held to. */
export interface Measurement {
  name: string;
  value: number | string | boolean | null;
  threshold: string;
  unit?: string;
  ok: boolean;
}

/** What a criterion's run answers. */
export interface Outcome {
  status: Status;
  measurements: Measurement[];
  notes: string[];
  error?: string;
}

/** A criterion in the report. */
export interface Entry extends Outcome {
  id: string;
  title: string;
  threshold: string;
  durationMs: number;
  /** The criterion's log, relative to the report. */
  log: string;
}

export interface Meta {
  mode: 'wsl' | 'native';
  app: string;
  version: string;
  started: Date;
  durationMs: number;
  only: string[] | null;
}

export interface Report {
  schema: 1;
  job: 'RB-099-02';
  mode: Meta['mode'];
  app: string;
  version: string;
  started: string;
  durationMs: number;
  only: string[] | null;
  summary: { passed: number; failed: number; skipped: number; total: number };
  ok: boolean;
  criteria: Entry[];
}

export function buildReport(entries: readonly Entry[], meta: Meta): Report {
  const count = (status: Status) => entries.filter((entry) => entry.status === status).length;
  const summary = {
    passed: count('passed'),
    failed: count('failed'),
    skipped: count('skipped'),
    total: entries.length,
  };
  return {
    schema: 1,
    job: 'RB-099-02',
    mode: meta.mode,
    app: meta.app,
    version: meta.version,
    started: meta.started.toISOString(),
    durationMs: meta.durationMs,
    only: meta.only,
    summary,
    ok: summary.total > 0 && summary.failed === 0,
    criteria: [...entries],
  };
}

/** 0 when nothing failed (an allowed skip included), 1 otherwise. */
export function exitCode(report: Report): number {
  return report.ok ? 0 : 1;
}

const seconds = (ms: number): string => `${(ms / 1000).toFixed(1)} s`;

function value(measurement: Measurement): string {
  const unit = measurement.unit === undefined ? '' : ` ${measurement.unit}`;
  return `${measurement.name} ${String(measurement.value)}${unit} (${measurement.threshold})`;
}

const cell = (text: string): string => text.replaceAll('|', '\\|').replaceAll('\n', ' ');

export function renderMarkdown(report: Report): string {
  const lines = [
    '# rBrowser end-to-end report',
    '',
    `Mode: ${report.mode}. App: ${cell(report.app)} (${report.version}). Started: ${report.started}.`,
    `Run time: ${seconds(report.durationMs)}. Result: ${report.summary.passed} passed, ` +
      `${report.summary.failed} failed, ${report.summary.skipped} skipped.`,
    '',
    '| Criterion | Title | Status | Measured (threshold) | Duration |',
    '| --- | --- | --- | --- | --- |',
  ];
  for (const entry of report.criteria) {
    const status = entry.status === 'failed' ? '**failed**' : entry.status;
    const measured = entry.measurements.map(value).join('; ') || '-';
    lines.push(
      `| ${entry.id} | ${cell(entry.title)} | ${status} | ${cell(measured)} | ${seconds(entry.durationMs)} |`,
    );
  }
  const told = report.criteria.filter((entry) => entry.error !== undefined || entry.notes.length > 0);
  if (told.length > 0) lines.push('', '## Notes and failures');
  for (const entry of told) {
    lines.push('', `### ${entry.id}`, '');
    if (entry.error !== undefined) lines.push(`Error: ${entry.error}`, '');
    for (const note of entry.notes) lines.push(`- ${note}`);
    lines.push(`- Log: \`${entry.log}\``);
  }
  return `${lines.join('\n')}\n`;
}

/** `text` with every bearer token, 64-hex secret and `token=` parameter masked. */
export function redact(text: string): string {
  return text
    .replace(/(Bearer\s+)[^\s"',]+/g, '$1***')
    .replace(/\b[0-9a-fA-F]{64}\b/g, '***')
    .replace(/([?&]token=)[^&\s"']+/g, '$1***');
}
