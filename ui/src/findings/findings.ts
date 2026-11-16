// The findings panel without its view (RB-060-01): filtering, grouping by rule, "only in
// breakpoint ...", the highlight request and the plugin texts for the i18n catalogue.

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
