// The checks of a request, a collection run's report and a benchmark (RB-096-04), as the core
// has them (crates/rb-api-model/src/checks.rs, crates/rbrowser/src/api_client/collection.rs and
// bench.rs; JSON in camelCase), and the panel's small decisions about them.

export type Compare = 'equals' | 'contains';

export type Check =
  | { kind: 'status'; expected: string }
  | { kind: 'json'; path: string; compare: Compare; value: string }
  | { kind: 'header'; name: string; compare: Compare; value: string }
  | { kind: 'time'; belowMs: number };

export type CheckKind = Check['kind'];

export const CHECK_KINDS: readonly CheckKind[] = ['status', 'json', 'header', 'time'];
export const COMPARES: readonly Compare[] = ['equals', 'contains'];
/** The most checks of one request (`MAX_CHECKS`). */
export const MAX_CHECKS = 50;
/** The longest time bound, the longest timeout (ms). */
export const MAX_BELOW_MS = 600000;

/** A new check of `kind`, with the value most requests want. */
export function newCheck(kind: CheckKind): Check {
  switch (kind) {
    case 'status':
      return { kind, expected: '2xx' };
    case 'json':
      return { kind, path: '$.', compare: 'equals', value: '' };
    case 'header':
      return { kind, name: 'Content-Type', compare: 'contains', value: 'json' };
    case 'time':
      return { kind, belowMs: 1000 };
  }
}

/** The check's line after `# @rb-check `, as the core writes it. */
export function checkLine(check: Check): string {
  const token = (compare: Compare): string => (compare === 'equals' ? '==' : 'contains');
  switch (check.kind) {
    case 'status':
      return `status ${check.expected}`;
    case 'json':
      return `json ${check.path} ${token(check.compare)} ${check.value}`.trimEnd();
    case 'header':
      return `header ${check.name} ${token(check.compare)} ${check.value}`.trimEnd();
    case 'time':
      return `time < ${check.belowMs}`;
  }
}

/** Whether the check is of its form, as the core holds it (`Check::validate`). */
export function checkOk(check: Check): boolean {
  switch (check.kind) {
    case 'status':
      return /^[1-5](\d\d|xx)$/.test(check.expected);
    case 'json':
      return check.path.trim().startsWith('$') && !/[\r\n]/.test(check.path + check.value);
    case 'header':
      return /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/.test(check.name) && !/[\r\n]/.test(check.value);
    case 'time':
      return Number.isInteger(check.belowMs) && check.belowMs >= 1 && check.belowMs <= MAX_BELOW_MS;
  }
}

export type Problem = 'missing' | 'notJson' | 'badPath';

export interface Verdict {
  check: string;
  passed: boolean;
  actual?: string;
  problem?: Problem;
}

export interface Failure {
  kind: string;
  code?: string;
  params?: Record<string, string>;
}

export interface Ran {
  id: string;
  name: string;
  method: string;
  url: string;
  sentUrl?: string;
  status?: number;
  totalMs?: number;
  error?: Failure;
  checks?: Verdict[];
  passed: boolean;
}

export interface Tally {
  requests: number;
  passed: number;
  failed: number;
  errors: number;
  checks: number;
  checksFailed: number;
}

export interface Report {
  at: number;
  folder?: string;
  environment?: string;
  tally: Tally;
  requests: Ran[];
  cut: boolean;
}

export interface Summary {
  count: number;
  min: number;
  median: number;
  p95: number;
  max: number;
  mean: number;
}

export const PHASES = ['dns', 'connect', 'tls', 'firstByte', 'total'] as const;
export type Phase = (typeof PHASES)[number];

export interface Series {
  environment: string | null;
  sent: number;
  statuses: Record<string, number>;
  errors: Record<string, number>;
  phases: Record<Phase, Summary | null>;
}

export interface Bench {
  id: string;
  repeats: number;
  concurrency: number;
  series: Series[];
  comparison?: { medianDelta: number; p95Delta: number };
}

/** The requests of a run that did not pass, in its order. */
export function failedRuns(report: Report): Ran[] {
  return report.requests.filter((ran) => !ran.passed);
}

/** Counts by key as `200: 5, 503: 1`, the largest count first. */
export function counted(counts: Record<string, number>): string {
  return Object.entries(counts)
    .sort(([a, x], [b, y]) => y - x || a.localeCompare(b))
    .map(([key, count]) => `${key}: ${count}`)
    .join(', ');
}

/** A signed difference in milliseconds: `+12 ms`, `-3.5 ms`. */
export function delta(ms: number): string {
  const size = Math.abs(ms) < 10 ? Math.abs(ms).toFixed(1) : String(Math.round(Math.abs(ms)));
  return `${ms < 0 ? '-' : '+'}${size} ms`;
}
