// The panel's tabs over the breakpoints (RB-094-02), without DOM: the Breakpoints tab draws the
// answer of `breakpoint_diff` - the table element x breakpoint, an entry per element some views
// show and others lack or hide, grouped by kind, a dot per view and "missing in <views>" -, the
// Reading order tab the answer of `reading_order` - its findings under their view, a click marking
// both elements. A command's `$post` `{ type: 'show', tab }` names the tab to show.

import { type Answer, type Cell, fill, type Item, type MarkTarget, names, presenceOf, type Strings, type Tab, TABS, targetsOf, text } from './model';

/** The kinds of element compared, in the order the tab lists them (`outline.diff.elements`). */
export const KINDS = ['interactive', 'headings', 'landmarks'] as const;

/** One row of `breakpoint_diff`: an element some views show and others lack or hide. */
export interface DiffRow {
  kind: string;
  label: string;
  url?: string;
  views: Record<string, Cell>;
  shown: number[];
  missing: number[];
}

/** What `breakpoint_diff` answers. */
export interface DiffAnswer extends Answer {
  rows?: DiffRow[];
  elements?: string[];
}

/** The Breakpoints tab's entries: a head per kind, its elements below it. */
export function diffItems(strings: Strings, answer: DiffAnswer): Item[] {
  const rows = answer.rows ?? [];
  return KINDS.flatMap((kind) => {
    const own = rows.filter((row) => row.kind === kind);
    if (own.length === 0) return [];
    const head: Item = { depth: 0, label: text(strings, `panel.kind.${kind}`), notes: [], presence: [], targets: [] };
    return [
      head,
      ...own.map((row) => ({
        depth: 1,
        label: row.label,
        badge: text(strings, 'panel.missingIn', { views: names(answer.views, row.missing) }),
        notes: [],
        presence: presenceOf(answer.views, row.views),
        targets: targetsOf(row.views),
      })),
    ];
  });
}

/** The rules the Reading order tab lists. */
const ORDER_RULES = new Set(['reading-order', 'focus-order-jump']);

/** A finding's element and, for the reading order, the other one. */
function pairOf(finding: Answer['findings'][number]): MarkTarget[] {
  const targets: MarkTarget[] = [];
  if (finding.selector) targets.push({ view: finding.view, selector: finding.selector });
  const other = finding.data?.other?.selector;
  if (other) targets.push({ view: finding.view, selector: other });
  return targets;
}

/** The Reading order tab's entries: per view its findings, each marking its elements. */
export function orderItems(strings: Strings, answer: Answer): Item[] {
  return answer.views.flatMap((view) => {
    const own = answer.findings.filter((finding) => finding.view === view.id && ORDER_RULES.has(finding.rule));
    if (own.length === 0) return [];
    const children: Item[] = own.map((finding) => ({
      depth: 1,
      label: fill(strings, finding.message, finding.params),
      notes: [{ text: text(strings, `rules.${finding.rule}`), severity: finding.severity ?? 'warning' }],
      presence: [],
      targets: pairOf(finding),
    }));
    const head: Item = { depth: 0, label: view.label, notes: [], presence: [], targets: children.flatMap((child) => child.targets) };
    return [head, ...children];
  });
}

/** The tab a command's post asks for; none for any other message. */
export function showTab(message: unknown): Tab | undefined {
  if (!message || typeof message !== 'object') return undefined;
  const { type, tab } = message as { type?: unknown; tab?: unknown };
  return type === 'show' && typeof tab === 'string' && (TABS as readonly string[]).includes(tab) ? (tab as Tab) : undefined;
}
