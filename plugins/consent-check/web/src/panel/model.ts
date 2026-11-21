// The consent check panel's model (RB-094-05), without DOM: the answer of `check`, the texts of
// its findings, banner, caveats, lifetimes and differences, and the one status line. App.vue and
// PhaseView.vue draw it. Texts are catalogue messages (plurals included) through the shared
// texts.js.

// The shared texts.js is the panel's prelude (panel-ui); imported here too for the tests.
import '../../../../web-common/panel/texts.js';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';

export const PREFIX = 'plugin.consent-check.';
export const SECTIONS = ['before', 'after', 'findings'] as const;
export type Section = (typeof SECTIONS)[number];
export type Strings = Record<string, string>;
export type Severity = 'error' | 'warning' | 'info';
export type Kind = 'script' | 'pixel' | 'font' | 'iframe' | 'style' | 'fetch' | 'media' | 'other';

export interface HostRow {
  host: string;
  kind: Kind;
  count: number;
  allowed: boolean;
  example: string;
}

export interface CookieRow {
  name: string;
  domain: string;
  days: number | null;
  essential: boolean;
  thirdParty: boolean;
}

export interface Phase {
  hosts: HostRow[];
  firstParty: number;
  cookies: CookieRow[];
  storage: string[];
}

export interface Diff {
  hostsAdded: string[];
  hostsRemoved: string[];
  cookiesAdded: string[];
  cookiesRemoved: string[];
  storageAdded: string[];
  storageRemoved: string[];
}

export interface Finding {
  rule: string;
  severity: Severity;
  message: string;
  params?: [string, string][];
}

export interface Answer {
  mode: 'session' | 'view';
  url: string;
  site: string;
  view: number | null;
  notice: string;
  noticeText: string;
  caveats: string[];
  rejectStep: boolean;
  banner: { cmp: string | null; name: string | null; visible: boolean; rejected: boolean };
  session: { fresh: boolean; discarded: boolean | null };
  before: Phase;
  afterReject: Phase | null;
  diff: Diff | null;
  findings: Finding[];
  summary: { error: number; warning: number };
  exported?: string;
}

/** The text of `key` under the plugin's prefix, its parameters filled; the key when missing. */
export function text(strings: Strings, key: string, params: Record<string, string | number> = {}): string {
  return PanelTexts.format(strings[PREFIX + key] ?? key, params);
}

/** A finding's sentence: its message key with its parameters. */
export function findingText(strings: Strings, finding: Finding): string {
  const params = Object.fromEntries(finding.params ?? []);
  return PanelTexts.format(strings[finding.message] ?? finding.message, params);
}

const RANK: Record<Severity, number> = { error: 0, warning: 1, info: 2 };

/** The findings, errors first, otherwise in the answer's order. */
export function sortedFindings(answer: Answer): Finding[] {
  return [...answer.findings].sort((a, b) => RANK[a.severity] - RANK[b.severity]);
}

/** The banner line: rejected, shown, found but hidden, or none. */
export function bannerText(strings: Strings, answer: Answer): string {
  const { banner } = answer;
  const name = banner.name ?? banner.cmp ?? '';
  if (!banner.cmp) return text(strings, 'panel.banner.none');
  if (banner.rejected) return text(strings, 'panel.banner.rejected', { name });
  return text(strings, banner.visible ? 'panel.banner.shown' : 'panel.banner.hidden', { name });
}

/** The sentences of the run's caveats. */
export function caveatTexts(strings: Strings, answer: Answer): string[] {
  return answer.caveats.map((caveat) => text(strings, `panel.caveat.${caveat}`));
}

/** A cookie's lifetime: whole days, or the browser session. */
export function lifetime(strings: Strings, days: number | null): string {
  return days === null ? text(strings, 'panel.cookieSession') : text(strings, 'panel.cookieLifetime', { days });
}

/** The kind of a request in words. */
export function kindText(strings: Strings, kind: string): string {
  return text(strings, `panel.kind.${kind}`);
}

/** A host row's kind and count. */
export function hostLabel(strings: Strings, row: HostRow): string {
  return `${kindText(strings, row.kind)}, ${text(strings, 'panel.count', { count: row.count })}`;
}

export interface DiffGroup {
  id: string;
  title: string;
  items: string[];
}

/** `host kind` as `host (kind)`. */
function hostItem(strings: Strings, key: string): string {
  const at = key.lastIndexOf(' ');
  return at < 0 ? key : `${key.slice(0, at)} (${kindText(strings, key.slice(at + 1))})`;
}

/** The differences after "Reject all" that are not empty. */
export function diffGroups(strings: Strings, answer: Answer): DiffGroup[] {
  const diff = answer.diff;
  if (!diff) return [];
  const groups: DiffGroup[] = [
    { id: 'hostsAdded', title: text(strings, 'panel.diff.hostsAdded'), items: diff.hostsAdded.map((key) => hostItem(strings, key)) },
    { id: 'hostsRemoved', title: text(strings, 'panel.diff.hostsRemoved'), items: diff.hostsRemoved.map((key) => hostItem(strings, key)) },
    { id: 'cookiesAdded', title: text(strings, 'panel.diff.cookiesAdded'), items: diff.cookiesAdded },
  ];
  return groups.filter((group) => group.items.length > 0);
}

/** The status line: a failure, the run, the export written, the findings reported. */
export function statusOf(
  strings: Strings,
  state: { failure: string | null; loading: boolean; answer: Answer | null; exported: string | null; reported: number | null },
): StatusLine | null {
  if (state.failure) return { tone: 'error', text: state.failure };
  if (state.loading) return { tone: 'busy', text: text(strings, 'panel.loading') };
  if (state.exported) return { tone: 'ok', text: text(strings, 'panel.exported', { path: state.exported }) };
  if (state.reported !== null) return { tone: 'ok', text: text(strings, 'panel.reported', { count: state.reported }) };
  return null;
}
