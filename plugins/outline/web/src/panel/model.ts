// The panel's model, without DOM: the texts, the answer of `get` turned into tree items - one per
// row, with what each view shows and the findings at it -, nested for Nuxt UI's tree (`nest`,
// RB-080-85), and the link check's batches. App.vue draws it. Texts are catalogue messages
// (plurals included) and a failure is a sentence, through the shared texts.js (RB-080-90).

// The shared texts.js is the panel's prelude (panel-ui); imported here too for the tests.
import '../../../../web-common/panel/texts.js';
import type { StatusLine, Tone } from '../../../../web-common/panel-ui/src/status';

export const PREFIX = 'plugin.outline.';
export const TABS = ['headings', 'landmarks', 'links', 'images', 'forms', 'focus', 'axtree', 'custom'] as const;
export type Tab = (typeof TABS)[number];
export type Strings = Record<string, string>;

/** The text `key` below `plugin.outline.` with `{name}` filled in; the key when it is missing. */
export function text(strings: Strings, key: string, params: Record<string, string | number> = {}): string {
  return PanelTexts.format(strings[PREFIX + key] ?? key, params);
}

/** The text of the full key `key` (a finding's message) with `params`; the key when unknown. */
export function fill(strings: Strings, key: string, params: Record<string, string>): string {
  return PanelTexts.format(strings[key] ?? key, params);
}

/** A failed call as one sentence: its code's text (the plugin's or the host's) and its reason's,
 * never the code (RB-080-90). */
export function errorText(strings: Strings, code: string, params: Record<string, string> = {}): string {
  return PanelTexts.errorText(strings, code, params);
}

export interface View {
  id: number;
  label: string;
  error?: string;
}

export interface Cell {
  index: number;
  visible: boolean;
  hidden: boolean;
  selector: string;
  path: string;
}

export interface Row {
  item: Record<string, unknown>;
  views: Record<string, Cell>;
  only?: number[];
}

export interface Finding {
  rule: string;
  message: string;
  params: Record<string, string>;
  view: number;
  path?: string;
}

export interface AxNode {
  depth: number;
  role: string;
  name: string;
  backendNodeId?: number;
}

/** What `get` answers for one tab. */
export interface Answer {
  views: View[];
  findings: Finding[];
  sections?: Record<string, unknown>;
}

/** A view's state of a row, for the dots. */
export type Presence = 'visible' | 'hidden' | 'missing';

/** Where a click marks: a selector or an accessibility node per view. */
export type MarkTarget = { view: number; selector: string } | { view: number; backendNodeId: number };

export interface Item {
  depth: number;
  label: string;
  /** "only Desktop", "hidden in every view", ... */
  badge?: string;
  /** Finding messages at this entry. */
  notes: string[];
  presence: { view: View; state: Presence }[];
  targets: MarkTarget[];
  /** The link's URL, for the link check. */
  href?: string;
}

const RULES_OF: Record<string, Tab> = {
  'heading-skip': 'headings',
  'multiple-h1': 'headings',
  'landmark-main-missing': 'landmarks',
  'link-empty': 'links',
  'link-broken': 'links',
  'img-alt-missing': 'images',
  'form-label-missing': 'forms',
};

/** The findings of `tab`'s rules. */
export function findingsOf(answer: Answer, tab: Tab): Finding[] {
  return answer.findings.filter((finding) => RULES_OF[finding.rule] === tab);
}

function names(views: View[], ids: number[]): string {
  return ids.map((id) => views.find((view) => view.id === id)?.label ?? String(id)).join(', ');
}

function badge(strings: Strings, views: View[], row: Row): string | undefined {
  if (row.only === undefined) return undefined;
  if (row.only.length === 0) return text(strings, 'panel.hiddenEverywhere');
  return text(strings, 'panel.only', { views: names(views, row.only) });
}

const s = (value: unknown): string => (typeof value === 'string' ? value : value == null ? '' : String(value));

/** One row's line, by section. */
export function label(strings: Strings, tab: Tab, item: Record<string, unknown>): string {
  switch (tab) {
    case 'headings':
      return `H${s(item.level)} ${s(item.text) || text(strings, 'panel.noText')}`;
    case 'landmarks':
      return [s(item.role), s(item.name) && `"${s(item.name)}"`, `<${s(item.tag)}>`].filter(Boolean).join(' ');
    case 'links': {
      const flags = [
        item.internal ? '' : text(strings, 'panel.external'),
        item.blankUnsafe ? text(strings, 'panel.blankUnsafe') : '',
        item.nofollow ? 'nofollow' : '',
      ].filter(Boolean);
      const name = s(item.text) || text(strings, 'panel.noText');
      return `${name} -> ${s(item.href)}${flags.length ? ` (${flags.join(', ')})` : ''}`;
    }
    case 'images': {
      const alt = item.alt === null ? text(strings, 'panel.altMissing') : `alt="${s(item.alt)}"`;
      const size = `${s(item.width)}x${s(item.height)} / ${s(item.naturalWidth)}x${s(item.naturalHeight)}`;
      return `${alt} ${s(item.src)} (${size}${item.lazy ? ', lazy' : ''})`;
    }
    case 'forms': {
      const name = s(item.label) || text(strings, 'panel.noLabel');
      const extra = [s(item.type), s(item.name) && `name=${s(item.name)}`, item.required ? 'required' : '', s(item.autocomplete) && `autocomplete=${s(item.autocomplete)}`];
      return `${name} [${extra.filter(Boolean).join(', ')}]`;
    }
    case 'focus':
      return `${s(item.role) || s(item.tag)} ${s(item.name) || text(strings, 'panel.noText')}`;
    default:
      return s(item.text) || s(item.label);
  }
}

function presence(views: View[], row: Row): Item['presence'] {
  return views.map((view) => {
    const cell = row.views[String(view.id)];
    return { view, state: !cell ? 'missing' : cell.visible ? 'visible' : 'hidden' };
  });
}

function notesAt(strings: Strings, findings: Finding[], row: Row): string[] {
  return findings
    .filter((finding) => finding.path && Object.entries(row.views).some(([id, cell]) => Number(id) === finding.view && cell.path === finding.path))
    .map((finding) => fill(strings, finding.message, finding.params))
    .filter((note, at, all) => all.indexOf(note) === at);
}

function targetsOf(row: Row): MarkTarget[] {
  return Object.entries(row.views).map(([id, cell]) => ({ view: Number(id), selector: cell.selector }));
}

function rowItems(strings: Strings, tab: Tab, answer: Answer, rows: Row[], depthOf: (row: Row) => number): Item[] {
  const findings = findingsOf(answer, tab);
  return rows.map((row) => ({
    depth: depthOf(row),
    label: label(strings, tab, row.item),
    badge: badge(strings, answer.views, row),
    notes: notesAt(strings, findings, row),
    presence: presence(answer.views, row),
    targets: targetsOf(row),
    href: tab === 'links' ? s(row.item.href) : undefined,
  }));
}

/** The tree items of `tab` from `get`'s answer; `axView` picks the view whose accessibility tree
 * is shown. */
export function items(strings: Strings, tab: Tab, answer: Answer, axView?: number): Item[] {
  const section = (answer.sections?.[tab] ?? {}) as { rows?: Row[]; selectors?: unknown[]; views?: Record<string, unknown> };
  if (tab === 'headings') {
    // Nested by level: a heading holds the deeper ones after it.
    return rowItems(strings, tab, answer, section.rows ?? [], (row) => Math.max(0, Number(row.item.level ?? 1) - 1));
  }
  if (tab === 'axtree') {
    const view = axView ?? answer.views[0]?.id;
    const tree = (section.views?.[String(view)] ?? {}) as { nodes?: AxNode[] };
    return (tree.nodes ?? []).map((node) => ({
      depth: node.depth,
      label: `${node.role} ${node.name}`.trim(),
      notes: [],
      presence: [],
      targets: node.backendNodeId && view !== undefined ? [{ view, backendNodeId: node.backendNodeId }] : [],
    }));
  }
  if (tab === 'custom') {
    const selectors = (section.selectors ?? []) as { label: string; selector: string; invalid: boolean; counts: Record<string, number>; rows: Row[] }[];
    return selectors.flatMap((group) => {
      const counts = answer.views.map((view) => `${view.label} ${group.counts[String(view.id)] ?? 0}`).join(', ');
      const head: Item = {
        depth: 0,
        label: `${group.label} (${group.selector})`,
        badge: group.invalid ? text(strings, 'panel.invalidSelector') : counts,
        notes: [],
        presence: [],
        targets: [],
      };
      return [head, ...rowItems(strings, tab, answer, group.rows, () => 1)];
    });
  }
  return rowItems(strings, tab, answer, section.rows ?? [], () => 0);
}

/** The link check's batches: the distinct `http(s)` URLs without fragment, `size` at a time. */
export function batches(hrefs: (string | undefined)[], size: number): string[][] {
  const urls = [...new Set(hrefs.filter((href): href is string => !!href && /^https?:\/\//i.test(href)).map((href) => href.split('#')[0] ?? href))];
  const step = Math.max(1, Math.min(4, Math.floor(size) || 1));
  const out: string[][] = [];
  for (let at = 0; at < urls.length; at += step) out.push(urls.slice(at, at + step));
  return out;
}

/** An item of the tree as Nuxt UI's `UTree` takes it: the item, a key and the deeper items after
 * it as its children, open by default. */
export interface TreeNode {
  key: string;
  item: Item;
  label: string;
  defaultExpanded: boolean;
  children?: TreeNode[];
}

/** `items` (flat, each with its depth) nested: an item holds the deeper ones after it. */
export function nest(items: Item[]): TreeNode[] {
  const roots: TreeNode[] = [];
  const open: { depth: number; node: TreeNode }[] = [];
  items.forEach((item, index) => {
    const node: TreeNode = { key: String(index), item, label: item.label, defaultExpanded: true };
    while (open.length > 0 && (open.at(-1)?.depth ?? 0) >= item.depth) open.pop();
    const parent = open.at(-1)?.node;
    if (parent) (parent.children ??= []).push(node);
    else roots.push(node);
    open.push({ depth: item.depth, node });
  });
  return roots;
}

const URGENCY: Tone[] = ['error', 'busy', 'warn', 'ok', 'info'];

/** The panel's one status line from its parts (a failure, the views without an outline, the
 * marks, the link check, a cut answer): their texts in order, the tone of the most urgent. */
export function statusLine(parts: (StatusLine | null | undefined)[]): StatusLine | null {
  const said = parts.filter((part): part is StatusLine => !!part && part.text !== '');
  if (said.length === 0) return null;
  const tone = URGENCY.find((one) => said.some((part) => part.tone === one)) ?? 'info';
  return { tone, text: said.map((part) => part.text).join(' ') };
}
