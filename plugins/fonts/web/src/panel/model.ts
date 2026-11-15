// The Fonts panel's model (RB-070-13): the `fonts_list` answer turned into the rows the panel
// shows - a row per family with its source, use per view and rendered state, the family's
// detail, its findings, and the font sizes of each text role across the breakpoints. No DOM here,
// so it is tested on its own. Texts are formatted as panel-ui formats them (ICU plurals, RB-080-83).

import { format } from '../../../../web-common/panel-ui/src/format';

export type Strings = Record<string, string>;
const PREFIX = 'plugin.fonts.';

export interface ViewInfo {
  id: number;
  label: string;
  width: number;
  engine: string;
  error?: string;
  rendered?: string;
}

export interface FaceEntry {
  weight: string;
  style: string;
  display: string;
  unicodeRange: string;
  status: { view: number; state: string }[];
  src: string | null;
  format: string;
  kb: number | null;
  preloaded: boolean;
}

export interface Family {
  family: string;
  provider: string;
  host: string;
  webFont: boolean;
  formats: string[];
  variable: boolean;
  faces: FaceEntry[];
  fileKb: number | null;
  usage: { view: number; elements: number; weights: number[]; styles: string[]; sizes: number[] }[];
  rendered: { view: number; state: string; as: string[] }[];
}

export interface Finding {
  rule: string;
  severity: string;
  message: string;
  params: Record<string, string>;
  view: number | null;
  path: string | null;
}

export interface RoleSeries {
  role: string;
  views: { view: number; width: number; min: number; max: number }[];
  growth: number | null;
  scaled: boolean | null;
  jump: boolean;
}

export interface Answer {
  views: ViewInfo[];
  families: Family[];
  roles: RoleSeries[];
  findings?: Finding[];
  cut?: boolean;
}

/** The text of `plugin.fonts.<key>` with its placeholders and plurals filled; the key when missing. */
export function text(strings: Strings, key: string, params: Record<string, string | number> = {}): string {
  return format(strings[PREFIX + key] ?? key, params);
}

/** A finding's message: its own key (`plugin.fonts.findings.*`) with its parameters. */
export function message(strings: Strings, finding: Finding): string {
  return format(strings[finding.message] ?? finding.message, finding.params);
}

/** The label of view `id`. */
export function viewLabel(answer: Answer, id: number): string {
  return answer.views.find((view) => view.id === id)?.label ?? `#${id}`;
}

/** The worst rendered state of a family over the views: `fallback`, `glyphs`, `ok`, or none. */
export function renderedState(family: Family): string | undefined {
  for (const state of ['fallback', 'glyphs', 'ok', 'unsupported', 'unknown']) {
    if (family.rendered.some((entry) => entry.state === state)) return state;
  }
  return undefined;
}

/** Whether a finding is about `family`. */
export function concerns(finding: Finding, family: string): boolean {
  const name = family.toLowerCase();
  return ['family', 'declared'].some((key) => (finding.params[key] ?? '').toLowerCase() === name);
}

/** The findings about `family`. */
export function findingsFor(answer: Answer, family: string): Finding[] {
  return (answer.findings ?? []).filter((finding) => concerns(finding, family));
}

/** One row of the family list. */
export interface Row {
  family: string;
  provider: string;
  elements: number;
  rendered?: string;
  findings: number;
  /** Used per view: `Desktop 12`. */
  perView: string;
}

/** The rows of the family list: the families in use first (most text first), then the declared
 * ones nobody uses. */
export function rows(answer: Answer): Row[] {
  const list = answer.families.map((family): Row => {
    const elements = family.usage.reduce((sum, usage) => sum + usage.elements, 0);
    return {
      family: family.family,
      provider: family.provider,
      elements,
      rendered: renderedState(family),
      findings: findingsFor(answer, family.family).length,
      perView: family.usage.map((usage) => `${viewLabel(answer, usage.view)} ${usage.elements}`).join(' · '),
    };
  });
  return list.sort((a, b) => b.elements - a.elements);
}

/** Whether the use table needs its Rendered column: some view knows how the family was drawn
 * (Chromium answers `unknown` where it cannot tell, which said nothing in every row). */
export function renderedKnown(family: Family): boolean {
  return family.rendered.some((entry) => entry.state !== 'unknown');
}

/** A face's states over the views, grouped by state in view order; one entry without views only
 * when every view that was read (not a failed or Firefox one) says the same. */
export function faceStates(answer: Answer, face: FaceEntry): { state: string; views: string[] }[] {
  const groups = new Map<string, string[]>();
  for (const entry of face.status) groups.set(entry.state, [...(groups.get(entry.state) ?? []), viewLabel(answer, entry.view)]);
  const list = [...groups].map(([state, views]) => ({ state, views }));
  const read = answer.views.filter((view) => !view.error).length;
  const one = list[0];
  return list.length === 1 && one && one.views.length >= read ? [{ state: one.state, views: [] }] : list;
}

/** The views the breakpoint table shows: those with a size for some role (a Firefox view has
 * none and was a column of dashes). */
export function roleViews(answer: Answer): ViewInfo[] {
  return answer.views.filter((view) => answer.roles.some((series) => series.views.some((point) => point.view === view.id)));
}

/** The cells of the breakpoint table: per role the largest size in each of `views`, in order. */
export function roleCells(answer: Answer, series: RoleSeries, views: ViewInfo[] = answer.views): string[] {
  return views.map((view) => {
    const point = series.views.find((entry) => entry.view === view.id);
    return point ? `${point.max}` : '–';
  });
}
