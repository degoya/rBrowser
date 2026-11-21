// The findings panel without its view (RB-060-01): filtering, grouping by rule, "only in
// breakpoint ...", the highlight request, "Show at N px" (RB-093-01) and the plugin texts for the
// i18n catalogue.

import type { ViewId } from '../ipc/protocol';
import type {
  CheckedView,
  FindingsRequest,
  FindingsRun,
  MergedFinding,
  Severity,
} from './types';

/** The severities, the most serious first. */
export const SEVERITIES: Severity[] = ['error', 'warning', 'info'];

const RANK: Record<Severity, number> = { info: 0, warning: 1, error: 2 };

export interface FindingsFilter {
  /** The severities shown. */
  severities: Severity[];
  /** One plugin's findings, or every plugin's for `''`. */
  plugin: string;
  /** The findings of one view, or of every view for `null`. */
  view: ViewId | null;
}

export interface RuleGroup {
  ruleId: string;
  /** The highest severity of its findings. */
  severity: Severity;
  findings: MergedFinding[];
}

export function filterFindings(findings: MergedFinding[], filter: FindingsFilter): MergedFinding[] {
  return findings.filter(
    (finding) =>
      filter.severities.includes(finding.severity) &&
      (filter.plugin === '' || finding.plugins.includes(filter.plugin)) &&
      (filter.view === null || finding.views.includes(filter.view)),
  );
}

/** The findings by rule id, in the order the list has them (highest severity first). */
export function groupByRule(findings: MergedFinding[]): RuleGroup[] {
  const groups = new Map<string, RuleGroup>();
  for (const finding of findings) {
    const group = groups.get(finding.ruleId);
    if (group === undefined) {
      groups.set(finding.ruleId, { ruleId: finding.ruleId, severity: finding.severity, findings: [finding] });
    } else {
      group.findings.push(finding);
      if (RANK[finding.severity] > RANK[group.severity]) {
        group.severity = finding.severity;
      }
    }
  }
  return [...groups.values()];
}

/** The views of `finding` when it is not in every view the run checked; `null` when it is. */
export function onlyIn(finding: MergedFinding, run: FindingsRun): CheckedView[] | null {
  if (run.views.length === 0 || run.views.every((view) => finding.views.includes(view.id))) {
    return null;
  }
  return run.views.filter((view) => finding.views.includes(view.id));
}

export function countBySeverity(findings: MergedFinding[]): Record<Severity, number> {
  const counts: Record<Severity, number> = { error: 0, warning: 0, info: 0 };
  for (const finding of findings) {
    counts[finding.severity] += 1;
  }
  return counts;
}

/** Every plugin that reported something or ran, sorted. */
export function pluginsOf(run: FindingsRun): string[] {
  const plugins = new Set(run.plugins);
  for (const finding of run.findings) {
    finding.plugins.forEach((plugin) => plugins.add(plugin));
  }
  return [...plugins].sort();
}

/** The request that marks `finding`'s element in its views; `null` without an element. */
export function highlightRequest(finding: MergedFinding): FindingsRequest | null {
  const selector = finding.element?.selector ?? finding.element?.path;
  if (selector === undefined || selector === '') {
    return null;
  }
  return { type: 'highlightFinding', findingId: finding.id, selector, views: [...finding.views] };
}

/** The widths a plugin may lease (the host's `MIN_PLUGIN_WIDTH` and `MAX_PLUGIN_WIDTH`). */
const MIN_WIDTH = 120;
const MAX_WIDTH = 3840;

/**
 * "Show at N px" of a finding whose `data` names a whole `width` (the width sweep's, RB-093-01):
 * the reporting plugin's `show` command (`plugin.<id>.show`) on the finding's first view, which
 * holds the view at that width through the host's width lease until Escape, Cancel or its time.
 * `null` without such a width, a view, or with more than one plugin reporting it.
 */
export function showWidthRequest(finding: MergedFinding): { width: number; request: FindingsRequest } | null {
  const { data } = finding;
  const width = typeof data === 'object' && data !== null && !Array.isArray(data) ? (data as { width?: unknown }).width : undefined;
  if (typeof width !== 'number' || !Number.isInteger(width) || width < MIN_WIDTH || width > MAX_WIDTH) {
    return null;
  }
  const [view] = finding.views;
  if (view === undefined || finding.plugins.length !== 1) {
    return null;
  }
  const id = `plugin.${finding.plugins[0]!.replace(/-/g, '_')}.show`;
  return { width, request: { type: 'runCommand', id, args: { target: view, width } } };
}

type Nested = { [key: string]: string | Nested };

/** Dotted keys (`plugin.seo.findings.alt`) as the nested catalogue vue-i18n merges; a key that
 * would sit below a text is left out. */
export function nestMessages(messages: Record<string, string>): Nested {
  const root: Nested = {};
  for (const [key, text] of Object.entries(messages).sort(([a], [b]) => a.localeCompare(b))) {
    const parts = key.split('.');
    const last = parts.pop();
    let node: Nested | undefined = root;
    for (const part of parts) {
      const next: string | Nested | undefined = node[part];
      if (typeof next === 'string') {
        node = undefined;
        break;
      }
      node = next ?? (node[part] = {});
    }
    if (node !== undefined && last !== undefined && node[last] === undefined) {
      node[last] = text;
    }
  }
  return root;
}

/** The index the focus moves to from `current` by `step` in a list of `length`, held to its ends;
 * `-1` for an empty list. */
export function moveFocus(current: number, length: number, step: 1 | -1): number {
  if (length === 0) {
    return -1;
  }
  if (current < 0) {
    return 0;
  }
  return Math.min(length - 1, Math.max(0, current + step));
}

/** A filter's choice while the run still offers it; all (`''`) once it does not (RB-094-14). */
export function keepChoice<T extends string | number>(choice: T | '', choices: readonly T[]): T | '' {
  return choice !== '' && choices.includes(choice) ? choice : '';
}

/** The most of a plugin's own failure reason a sentence shows. */
export const REASON_LIMIT = 200;

/** The parameters of a failure's sentence (`code`), so none shows raw (RB-080-98): the code's
 *  own, then the host's names of the plugin, so a plugin cannot rename itself. The `reason` is
 *  the plugin's own text only under the plugin's own code (`plugin.<id>.`), cut; a host code's
 *  (`plugins.error.crashed`) is raw engine text and stays out. */
export function failureParams(
  code: string,
  plugin: string,
  params: Record<string, string> | undefined,
): Record<string, string> {
  const own = code.startsWith(`plugin.${plugin}.`);
  const reason = own ? cutText(params?.reason ?? '', REASON_LIMIT) : '';
  return { ...params, id: plugin, plugin, reason };
}

/** `text` at most `limit` characters, an ellipsis where it was cut. */
function cutText(text: string, limit: number): string {
  const chars = [...text];
  return chars.length <= limit ? text : `${chars.slice(0, limit - 1).join('')}\u2026`;
}
