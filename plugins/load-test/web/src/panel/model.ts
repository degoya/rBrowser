// The load test panel's model (RB-096-07): the shapes the core's ops answer
// (crates/rbrowser/src/load_test), a new scenario, the profile as the editor shows it - a
// constant number of users for a time, or a ramp up, hold and down - and the numbers' formats.

export const HOST_NOT_CONFIRMED = 'plugin.load-test.errors.host_not_confirmed';

export type Order = 'sequence' | 'weighted';
export type Standing = 'own' | 'local' | 'confirmed' | 'unconfirmed';
export type Shape = 'constant' | 'ramp';

export interface Step {
  request: string;
  weight: number;
  thinkMs: { min: number; max: number };
}

export interface Profile {
  users: number;
  rampUpS: number;
  holdS: number;
  rampDownS: number;
  rps?: number;
}

export interface AbortRules {
  errorRate?: number;
  p95Ms?: number;
  windowS: number;
  minRequests: number;
}

export interface Scenario {
  name: string;
  environment?: string;
  order: Order;
  steps: Step[];
  variables?: Record<string, string[]>;
  profile: Profile;
  abort: AbortRules;
}

export interface Listed {
  id: string;
  name: string;
  problem?: string;
}

export interface SavedRequest {
  id: string;
  name: string;
  method: string;
  url: string;
}

export interface Limits {
  maxUsers: number;
  maxRps: number;
  maxDurationS: number;
}

export interface Latency {
  p50: number;
  p95: number;
  p99: number;
  min: number;
  max: number;
  mean: number;
}

export interface Totals {
  requests: number;
  errors: number;
  errorRate: number;
  rps: number;
  latency: Latency | null;
}

export interface Point {
  t: number;
  requests: number;
  errors: number;
  users: number;
  p50: number | null;
  p95: number | null;
  p99: number | null;
}

export interface Abort {
  rule: 'errorRate' | 'p95Ms';
  value: number;
  threshold: number;
}

/** A run as the core pushes it every second (`{loadTest: Snapshot}`) and at its end. */
export interface Snapshot {
  running: boolean;
  scenario: string;
  name: string;
  targets: string[];
  elapsedS: number;
  durationS: number;
  users: number;
  targetUsers: number;
  maxUsers: number;
  rpsLimit: number;
  totals: Totals;
  errors: Record<string, number>;
  series: Point[];
  end: 'completed' | 'stopped' | 'aborted' | 'closed' | null;
  abort: Abort | null;
  reportId: string | null;
}

export interface Confirm {
  targets: string[];
  users: number;
  rps: number;
  durationS: number;
}

export interface Summary {
  scenario: string;
  name: string;
  environment: string | null;
  targets: { host: string; standing: Standing }[];
  unconfirmed: string[];
  users: number;
  rps: number;
  durationS: number;
  rampUpS: number;
  holdS: number;
  rampDownS: number;
  steps: number;
  confirm: Confirm;
}

export interface ReportRow {
  id: string;
  name: string;
  scenario: string;
  startedAt: string;
  end: string;
  requests: number;
  rps: number;
  errorRate: number;
  p95: number | null;
}

export interface Figures {
  id: string;
  name: string;
  startedAt: string;
  requests: number | null;
  rps: number | null;
  errorRate: number | null;
  p50: number | null;
  p95: number | null;
  p99: number | null;
}

export interface Compared {
  a: Figures;
  b: Figures;
  delta: Record<'requests' | 'rps' | 'errorRate' | 'p50' | 'p95' | 'p99', number | null>;
}

export interface State {
  trusted: boolean;
  environment: string | null;
  environments: string[];
  requests: SavedRequest[];
  scenarios: Listed[];
  limits: Limits;
  hosts: { own: string[]; local: string[]; confirmed: string[] };
  running: boolean;
  run: Snapshot | null;
  reports: ReportRow[];
}

/** A new scenario: the first saved request, a constant run of a few users for a minute. */
export function emptyScenario(request?: string): Scenario {
  return {
    name: '',
    order: 'sequence',
    steps: [{ request: request ?? '', weight: 1, thinkMs: { min: 0, max: 0 } }],
    profile: { users: 10, rampUpS: 0, holdS: 60, rampDownS: 0 },
    abort: { windowS: 10, minRequests: 20 },
  };
}

/** A new step after `steps`. */
export function newStep(request?: string): Step {
  return { request: request ?? '', weight: 1, thinkMs: { min: 0, max: 0 } };
}

/** The profile's shape: a ramp when it climbs or falls. */
export function shapeOf(profile: Profile): Shape {
  return profile.rampUpS > 0 || profile.rampDownS > 0 ? 'ramp' : 'constant';
}

/** The run's length in seconds. */
export function durationOf(profile: Profile): number {
  return profile.rampUpS + profile.holdS + profile.rampDownS;
}

/** `profile` in `shape`: a constant run keeps the whole length as its hold; a ramp gets a tenth
 * of it up and down. */
export function withShape(profile: Profile, shape: Shape): Profile {
  if (shape === shapeOf(profile)) return profile;
  const total = durationOf(profile);
  if (shape === 'constant') return { ...profile, rampUpS: 0, holdS: total, rampDownS: 0 };
  const ramp = Math.max(1, Math.round(total / 10));
  return { ...profile, rampUpS: ramp, holdS: Math.max(1, total - 2 * ramp), rampDownS: ramp };
}

/** What in `scenario` passes a ceiling of `limits`, as the names of the fields. */
export function overLimits(scenario: Scenario, limits: Limits): ('users' | 'rps' | 'duration')[] {
  const over: ('users' | 'rps' | 'duration')[] = [];
  if (scenario.profile.users > limits.maxUsers) over.push('users');
  if (scenario.profile.rps !== undefined && scenario.profile.rps > limits.maxRps) over.push('rps');
  if (durationOf(scenario.profile) > limits.maxDurationS) over.push('duration');
  return over;
}

/** A scenario as the core stores it: a rate or an abort threshold left empty is left out. */
export function cleaned(scenario: Scenario): Scenario {
  const profile = { ...scenario.profile };
  if (profile.rps === undefined || !(profile.rps > 0)) delete profile.rps;
  const abort = { ...scenario.abort };
  if (abort.errorRate === undefined || !(abort.errorRate >= 0)) delete abort.errorRate;
  if (abort.p95Ms === undefined || !(abort.p95Ms > 0)) delete abort.p95Ms;
  const out: Scenario = { ...scenario, profile, abort };
  if (!out.environment) delete out.environment;
  if (out.variables && Object.keys(out.variables).length === 0) delete out.variables;
  return out;
}

/** Milliseconds as text: `12 ms`, `1.25 s`; a dash without a value. */
export function millis(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return '-';
  if (value >= 1000) return `${(value / 1000).toFixed(2)} s`;
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ms`;
}

/** A share as a percentage: `2.5 %`. */
export function percent(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return '-';
  return `${(value * 100).toFixed(value < 0.1 ? 1 : 0)} %`;
}

/** Seconds as `m:ss`. */
export function clock(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`;
}

/** The snapshot a panel event carries, if it carries one. */
export function snapshotOf(message: unknown): Snapshot | null {
  if (!message || typeof message !== 'object') return null;
  const value = (message as { loadTest?: unknown }).loadTest;
  if (!value || typeof value !== 'object') return null;
  const snapshot = value as Partial<Snapshot>;
  return typeof snapshot.running === 'boolean' && Array.isArray(snapshot.series) ? (value as Snapshot) : null;
}

/** The requests of the last closed second. */
export function lastRate(snapshot: Snapshot): number {
  return snapshot.series.at(-1)?.requests ?? 0;
}

/** The setting each failure names in its text (RB-094-21): the status line links there. */
const FAILURE_SETTINGS: Readonly<Record<string, string>> = {
  'plugin.load-test.errors.api_client_missing': 'plugins',
  'plugin.load-test.errors.too_many_hosts': 'load-test.allowedHosts',
  'plugin.load-test.errors.over_limit': 'load-test',
};

/** The setting the failure `code` names: over a limit the very setting its `setting` parameter
 * names (one of the plugin's own), else the failure's; `undefined` for none. */
export function failureSetting(code: string, params: Record<string, unknown> | undefined): string | undefined {
  const named = params?.setting;
  if (code === 'plugin.load-test.errors.over_limit' && typeof named === 'string' && /^load-test\.[A-Za-z]+$/.test(named)) return named;
  return FAILURE_SETTINGS[code];
}
