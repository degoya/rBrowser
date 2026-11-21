// The structured data panel's model (RB-090-96), without DOM: the answer of `inspect`, turned into
// Nuxt UI's tree - a node per JSON-LD object or Microdata / RDFa item, a property under it, a
// nested node or a reference under its property -, the findings at their nodes, the way to a
// node, the views to choose from and the one status line. App.vue and its parts draw it. Texts
// are catalogue messages (plurals included) through the shared texts.js.

// The shared texts.js is the panel's prelude (panel-ui); imported here too for the tests.
import '../../../../web-common/panel/texts.js';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';

export const PREFIX = 'plugin.structured-data.';
export const SECTIONS = ['tree', 'findings', 'raw'] as const;
export type Section = (typeof SECTIONS)[number];
export type Strings = Record<string, string>;
export type Severity = 'error' | 'warning' | 'info';
export type Syntax = 'json-ld' | 'microdata' | 'rdfa';

export interface Source {
  syntax: Syntax;
  block?: number;
  path: string;
}

export type Val =
  | { kind: 'text'; text: string }
  | { kind: 'number'; text: string }
  | { kind: 'bool'; value: boolean }
  | { kind: 'node'; node: Node }
  | { kind: 'ref'; atId: string; target: string | null };

export interface Prop {
  name: string;
  values: Val[];
}

export interface Node {
  id: string;
  types: string[];
  otherTypes?: string[];
  atId?: string;
  source: Source;
  pointer: string;
  props: Prop[];
}

export interface Block {
  index: number;
  path: string;
  text: string | null;
  textCut: boolean;
  error?: string;
}

export interface Finding {
  rule: string;
  severity: Severity;
  message: string;
  params: Record<string, string>;
  node: string | null;
  property: string | null;
  place: string;
}

export interface ViewRef {
  id: number;
  label: string;
  url: string;
}

export interface Answer {
  view: ViewRef;
  views: ViewRef[];
  url: string;
  vocabulary: string;
  richResults: boolean;
  minSeverity: Severity;
  blocks: Block[];
  items: Node[];
  findings: Finding[];
  summary: Record<Severity, number>;
  truncated: boolean;
  cut: boolean;
}

/** One entry of Nuxt UI's tree: a node, a property or a reference. */
export interface TreeItem {
  key: string;
  label: string;
  /** The node's syntax and `@id`, beside its types. */
  note?: string;
  /** The node an entry is (`n3`). */
  node?: string;
  /** The node a reference points to. */
  target?: string;
  /** The worst finding at the entry, and how many there are. */
  severity?: Severity;
  count?: number;
  defaultExpanded: boolean;
  children?: TreeItem[];
}

/** The icon and the colour of each severity, beside its word. */
export const SEVERITY_ICON: Record<Severity, string> = {
  error: 'i-lucide-circle-x',
  warning: 'i-lucide-triangle-alert',
  info: 'i-lucide-info',
};
const RANK: Record<Severity, number> = { error: 0, warning: 1, info: 2 };

/** The text `key` below `plugin.structured-data.` with `{name}` filled in; the key when missing. */
export function text(strings: Strings, key: string, params: Record<string, string | number> = {}): string {
  return PanelTexts.format(strings[PREFIX + key] ?? key, params);
}

/** A finding's sentence: its message (a full key) with its params. */
export function findingText(strings: Strings, finding: Finding): string {
  return PanelTexts.format(strings[finding.message] ?? finding.message, finding.params);
}

/** The findings at the node `id`. */
export function findingsAt(answer: Answer, id: string): Finding[] {
  return answer.findings.filter((finding) => finding.node === id);
}

/** The findings, errors first, otherwise in the page's order. */
export function sortedFindings(answer: Answer): Finding[] {
  return answer.findings.map((finding, index) => ({ finding, index })).sort((a, b) => RANK[a.finding.severity] - RANK[b.finding.severity] || a.index - b.index).map((one) => one.finding);
}

function worst(findings: Finding[]): Severity | undefined {
  return findings.reduce<Severity | undefined>((found, finding) => (found === undefined || RANK[finding.severity] < RANK[found] ? finding.severity : found), undefined);
}

function nodeLabel(strings: Strings, node: Node): string {
  const types = node.types.length > 0 ? node.types : (node.otherTypes ?? []);
  return types.length > 0 ? types.join(', ') : text(strings, 'panel.untyped');
}

function valueText(value: Val): string {
  switch (value.kind) {
    case 'text':
    case 'number':
      return value.text;
    case 'bool':
      return String(value.value);
    case 'ref':
      return value.atId;
    case 'node':
      return '';
  }
}

function index(items: Node[], into = new Map<string, Node>()): Map<string, Node> {
  for (const node of items) {
    into.set(node.id, node);
    for (const prop of node.props) index(prop.values.flatMap((value) => (value.kind === 'node' ? [value.node] : [])), into);
  }
  return into;
}

/** The answer's nodes as the tree's items. */
export function toTree(strings: Strings, answer: Answer): TreeItem[] {
  const nodes = index(answer.items);
  const refLabel = (value: Extract<Val, { kind: 'ref' }>): string => {
    const target = value.target ? nodes.get(value.target) : undefined;
    if (!target) return text(strings, 'panel.refElsewhere', { id: value.atId });
    return text(strings, 'panel.refTo', { target: `${nodeLabel(strings, target)} ${value.atId}`.trim() });
  };
  const item = (node: Node): TreeItem => {
    const at = findingsAt(answer, node.id);
    const note = [text(strings, `panel.syntax.${node.source.syntax}`), node.atId ?? ''].filter(Boolean).join(' ');
    const children = node.props.map((prop): TreeItem => {
      const key = `${node.id}/${prop.name}`;
      const own = at.filter((finding) => finding.property === prop.name);
      const marks = { severity: worst(own), count: own.length > 0 ? own.length : undefined };
      const [only] = prop.values;
      if (prop.values.length === 1 && only && only.kind !== 'node' && only.kind !== 'ref') {
        return { key, label: `${prop.name}: ${valueText(only)}`, defaultExpanded: false, ...marks };
      }
      const values = prop.values.map((value, position): TreeItem => {
        if (value.kind === 'node') return item(value.node);
        if (value.kind === 'ref') return { key: `${key}/${position}`, label: refLabel(value), target: value.target ?? undefined, defaultExpanded: false };
        return { key: `${key}/${position}`, label: valueText(value), defaultExpanded: false };
      });
      return { key, label: prop.name, defaultExpanded: true, children: values, ...marks };
    });
    return { key: node.id, node: node.id, label: nodeLabel(strings, node), note, severity: worst(at), count: at.length > 0 ? at.length : undefined, defaultExpanded: true, children };
  };
  return answer.items.map(item);
}

/** The keys of the entries above the node `id`, outermost first; `null` when no entry is it. */
export function ancestorsOf(items: TreeItem[], id: string, above: string[] = []): string[] | null {
  for (const one of items) {
    if (one.node === id) return above;
    const inner = ancestorsOf(one.children ?? [], id, [...above, one.key]);
    if (inner) return inner;
  }
  return null;
}

/** The findings by severity, as one sentence. */
export function summaryText(strings: Strings, answer: Answer): string {
  return text(strings, 'panel.summary', answer.summary);
}

/** The choice of view: the active one, then each view. */
export function viewItems(strings: Strings, answer: Answer | null): { label: string; value: number | 'active' }[] {
  return [{ label: text(strings, 'panel.activeView'), value: 'active' as const }, ...(answer?.views ?? []).map((view) => ({ label: view.label, value: view.id }))];
}

/** The setting that checks the rich results. */
const RICH_SETTING = 'structured-data.richResults';

/** The panel's one status line: a failure, the read on its way, or what the answer leaves out. */
export function statusOf(strings: Strings, state: { failure: string | null; loading: boolean; answer: Answer | null }): StatusLine | null {
  if (state.failure) return { tone: 'error', text: state.failure };
  if (state.loading) return { tone: 'busy', text: text(strings, 'panel.loading') };
  const answer = state.answer;
  if (!answer) return null;
  const notes = [answer.cut ? text(strings, 'panel.cut') : '', answer.truncated ? text(strings, 'panel.truncated') : '', answer.richResults ? '' : text(strings, 'panel.richOff')].filter(Boolean);
  if (notes.length === 0) return null;
  const line: StatusLine = { tone: 'warn', text: notes.join(' ') };
  // "Rich results are not checked (Settings > Checks > Structured data)" links there (RB-094-21).
  return answer.richResults ? line : { ...line, setting: { key: RICH_SETTING, label: text(strings, 'panel.setting_link') } };
}
