// The security header panel's model (RB-093-03), without DOM: the answer of the tool `compare`
// (header x environment) as the table's rows and columns, the environments' findings and the one
// status line. App.vue draws it. Texts are catalogue messages (plurals included) through the
// shared texts.js.

// The shared texts.js is the panel's prelude (panel-ui); imported here too for the tests.
import '../../../../web-common/panel/texts.js';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';

export const PREFIX = 'plugin.security-headers.';
export type Strings = Record<string, string>;
export type Severity = 'error' | 'warning' | 'info';

export interface Finding {
  rule: string;
  severity: Severity;
  message: string;
  params: Record<string, string>;
  path?: string | null;
}

export interface Failure {
  code?: string;
  params?: Record<string, string>;
}

export interface Environment {
  id: string;
  url: string;
  finalUrl?: string;
  status?: number;
  findings?: Finding[];
  error?: Failure;
}

export interface Row {
  header: string;
  cookie?: string;
  values: Record<string, string | null>;
  differs: boolean;
}

export interface Comparison {
  path: string;
  environments: Environment[];
  rows: Row[];
  differences: number;
}

/** One line of the table as App.vue draws it. */
export interface Line {
  key: string;
  label: string;
  differs: boolean;
  /** By environment id: the cell's text and whether the header was sent. */
  cells: Record<string, { text: string; sent: boolean }>;
}

const RANK: Record<Severity, number> = { error: 0, warning: 1, info: 2 };

/** The plugin's text `key` (below its prefix) with `params`. */
export function text(strings: Strings, key: string, params: Record<string, string | number> = {}): string {
  return PanelTexts.format(strings[PREFIX + key] ?? key, params);
}

/** A full key's text (a finding's message, a host failure's code) with `params`. */
export function keyText(strings: Strings, key: string, params: Record<string, string> = {}): string {
  return PanelTexts.format(strings[key] ?? key, params);
}

/** A finding's sentence. */
export function findingText(strings: Strings, finding: Finding): string {
  return keyText(strings, finding.message, finding.params);
}

/** Why an environment's page could not be read. */
export function failureText(strings: Strings, failure: Failure): string {
  return failure.code ? keyText(strings, failure.code, failure.params ?? {}) : text(strings, 'panel.failed');
}

/** The environments whose page was read. */
export function readEnvironments(comparison: Comparison): Environment[] {
  return comparison.environments.filter((environment) => !environment.error);
}

/** The table's lines: every row, or only those that differ. */
export function lines(strings: Strings, comparison: Comparison, onlyDifferences: boolean): Line[] {
  return comparison.rows
    .filter((row) => !onlyDifferences || row.differs)
    .map((row) => ({
      key: row.cookie === undefined ? row.header : `cookie:${row.cookie}`,
      label: row.cookie === undefined ? row.header : text(strings, 'panel.cookie', { name: row.cookie }),
      differs: row.differs,
      cells: Object.fromEntries(
        comparison.environments.map((environment) => {
          const value = row.values[environment.id] ?? null;
          if (environment.error) return [environment.id, { text: '', sent: false }];
          if (value === null) return [environment.id, { text: text(strings, 'panel.absent'), sent: false }];
          // A cookie set without Secure, HttpOnly or SameSite: a word, never an empty cell.
          if (value === '') return [environment.id, { text: text(strings, 'panel.noAttributes'), sent: false }];
          return [environment.id, { text: value, sent: true }];
        }),
      ),
    }));
}

/** An environment's findings, errors first, otherwise in the answer's order. */
export function sortedFindings(environment: Environment): Finding[] {
  return (environment.findings ?? [])
    .map((finding, index) => ({ finding, index }))
    .sort((a, b) => RANK[a.finding.severity] - RANK[b.finding.severity] || a.index - b.index)
    .map((one) => one.finding);
}

/** The panel's status line. */
export function statusOf(strings: Strings, state: { failure: string | null; loading: boolean; comparison: Comparison | null }): StatusLine | null {
  if (state.failure) return { tone: 'error', text: state.failure };
  if (state.loading) return { tone: 'busy', text: text(strings, 'panel.loading') };
  const comparison = state.comparison;
  if (!comparison) return null;
  const failed = comparison.environments.filter((environment) => environment.error).length;
  if (failed > 0) return { tone: 'warn', text: text(strings, 'panel.summary', { count: comparison.differences }) };
  if (comparison.differences === 0) return { tone: 'ok', text: text(strings, 'panel.same') };
  return { tone: 'info', text: text(strings, 'panel.summary', { count: comparison.differences }) };
}
