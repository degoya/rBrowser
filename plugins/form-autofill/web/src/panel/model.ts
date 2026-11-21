// The form autofill panel's logic without the DOM (RB-070-11, per field since RB-080-86): the
// arguments of a fill, and the answer of the tool `fill` (plugins/form-autofill/src/fill.rs) as a
// row per field instead of a column per view - the value once, the views it differs in counted -,
// the fields skipped everywhere apart, a section per form and the views not filled said once.

import type { StatusLine } from '../../../../web-common/panel-ui/src/status';

export type Translate = (key: string, params?: Record<string, unknown>) => string;

export interface AutofillField {
  key: string;
  label: string;
  kind: string;
  value: string;
  violation: string | null;
  profile: boolean;
  skip: string;
  status: string | null;
  valid: boolean | null;
  errors: string[];
  /** Since RB-080-86; older answers lack both. */
  required?: boolean;
  form?: number | null;
}

export interface AutofillForm {
  selector: string;
  name: string;
}

export interface AutofillView {
  view: number;
  label: string;
  breakpoint: string;
  engine: string;
  status: string;
  error?: string | null;
  locale?: string;
  url?: string;
  formValid?: boolean | null;
  blockedSubmits?: number;
  forms?: AutofillForm[];
  /** The controls the fill ran for: `selector` or `page` (RB-090-86). */
  scope?: string;
  fields: AutofillField[];
}

export interface AutofillResult {
  mode: 'valid' | 'invalid';
  seed: number;
  strategy: string | null;
  profile: string | null;
  selector?: string | null;
  views: AutofillView[];
}

export const STRATEGIES = ['mixed', 'empty', 'format', 'bounds', 'length', 'chars'] as const;
/** The largest seed the tool takes. */
export const MAX_SEED = 4294967295;
const MAX_SELECTOR = 1024;
/** The reasons of a skip the locale has words for. */
const REASONS = ['disabled', 'file', 'hidden', 'readonly', 'filled'];
/** Engine names are product names, never translated. */
const ENGINES: Record<string, string> = { chromium: 'Chromium', firefox: 'Firefox', webkit: 'WebKit' };

/** The profile names of the settings (`form-autofill.profiles`). */
export function profiles(settings: Record<string, unknown>): string[] {
  const list = settings['form-autofill.profiles'];
  if (!Array.isArray(list)) return [];
  return list
    .map((entry: unknown) => (entry && typeof entry === 'object' ? (entry as { name?: unknown }).name : null))
    .filter((name): name is string => typeof name === 'string' && name.length > 0);
}

export interface FillForm {
  strategy: string;
  selector: string;
  profile: string;
  seed: number | null | undefined;
}

/** The arguments of the tool `fill` from the panel's options; what is unset is left out. */
export function fillArgs(mode: 'valid' | 'invalid', form: FillForm): Record<string, unknown> {
  const args: Record<string, unknown> = { mode };
  if (mode === 'invalid' && (STRATEGIES as readonly string[]).includes(form.strategy)) args.strategy = form.strategy;
  const selector = form.selector.trim();
  if (selector) args.selector = selector.slice(0, MAX_SELECTOR);
  if (form.profile) args.profile = form.profile;
  const seed = form.seed;
  if (typeof seed === 'number' && Number.isInteger(seed) && seed >= 0 && seed <= MAX_SEED) args.seed = seed;
  return args;
}

/** A new seed in the range the plugin draws its own from (1 to 999999). */
export function newSeed(random: () => number = Math.random): number {
  return 1 + Math.min(Math.floor(random() * 999999), 999998);
}

/** A field's name without the page's own star when the field is required (the panel draws one). */
export function fieldName(field: AutofillField): { text: string; required: boolean } {
  const required = field.required === true;
  return { text: required ? field.label.replace(/\s*\*\s*$/, '') : field.label, required };
}

/** What a value says beside itself, in words (never by colour alone). */
export function marks(t: Translate, field: AutofillField): { text: string; color: 'error' | 'warning' | 'info' }[] {
  const out: { text: string; color: 'error' | 'warning' | 'info' }[] = [];
  if (field.valid === false) out.push({ text: t('panel.invalidMark'), color: 'error' });
  if (field.violation) out.push({ text: t('panel.broken', { violation: field.violation }), color: 'warning' });
  if (field.profile) out.push({ text: t('panel.fromProfile'), color: 'info' });
  return out;
}

/** A skip's reason in words; one the locale lacks as the plugin said it. */
export const reasonText = (t: Translate, reason: string): string => (REASONS.includes(reason) ? t('panel.reasons.' + reason) : reason);

/** One view's entry for a field: none when the field is not on its page. */
export interface Cell {
  view: AutofillView;
  field: AutofillField | null;
  /** It holds the field's common value. */
  same: boolean;
}

export interface FieldRow {
  key: string;
  /** The field as the first view with the common value has it. */
  field: AutofillField;
  /** The value most views hold. */
  value: string;
  /** The views filled that hold another value, skipped it or lack it. */
  differs: number;
  cells: Cell[];
}

export interface SkippedRow {
  key: string;
  field: AutofillField;
  reasons: string[];
}

export interface Section {
  /** The form's selector; null for the fields outside a form. */
  selector: string | null;
  name: string;
  rows: FieldRow[];
  skipped: SkippedRow[];
}

export interface Outcome {
  sections: Section[];
  /** The views filled, whose fields the rows compare. */
  filled: AutofillView[];
  /** The views not filled (another engine, a failure, nothing found). */
  others: AutofillView[];
  fields: number;
  skipped: number;
}

/** The forms of every view of `result`, once each, in the order they come. */
export function formsOf(result: AutofillResult): AutofillForm[] {
  const seen = new Map<string, AutofillForm>();
  for (const view of result.views) {
    for (const form of Array.isArray(view.forms) ? view.forms : []) {
      if (form && typeof form.selector === 'string' && form.selector && !seen.has(form.selector)) seen.set(form.selector, form);
    }
  }
  return [...seen.values()];
}

/** The value of a cell, or null when it holds none (skipped, or not on the page). */
const valueOf = (cell: AutofillField | null): string | null => (cell && !cell.skip ? cell.value : null);

/** The most frequent of `values` (the earliest on a tie). */
function common(values: string[]): string {
  const counts = new Map<string, number>();
  for (const value of values) counts.set(value, (counts.get(value) ?? 0) + 1);
  let best = values[0] ?? '';
  for (const [value, count] of counts) if (count > (counts.get(best) ?? 0)) best = value;
  return best;
}

/** The answer of `fill` as a row per field, grouped by form. */
export function outcome(result: AutofillResult, t: Translate): Outcome {
  const views = Array.isArray(result?.views) ? result.views : [];
  const filled = views.filter((view) => view.status === 'filled');
  const others = views.filter((view) => view.status !== 'filled');
  const forms = formsOf(result);
  const byKey = new Map<string, { cells: (AutofillField | null)[]; form: string | null }>();
  filled.forEach((view, at) => {
    for (const field of Array.isArray(view.fields) ? view.fields : []) {
      let entry = byKey.get(field.key);
      if (!entry) {
        entry = { cells: filled.map(() => null), form: null };
        byKey.set(field.key, entry);
      }
      entry.cells[at] = field;
      const form = typeof field.form === 'number' ? view.forms?.[field.form] : undefined;
      if (entry.form === null && form?.selector) entry.form = form.selector;
    }
  });
  const sections = new Map<string | null, Section>();
  const section = (selector: string | null): Section => {
    let found = sections.get(selector);
    if (!found) {
      const at = forms.findIndex((form) => form.selector === selector);
      const name = selector === null ? t('panel.outside') : forms[at]?.name || t('panel.formNumber', { number: at + 1 });
      found = { selector, name, rows: [], skipped: [] };
      sections.set(selector, found);
    }
    return found;
  };
  let fields = 0;
  let skipped = 0;
  for (const [key, entry] of byKey) {
    const values = entry.cells.map(valueOf).filter((value): value is string => value !== null);
    const first = entry.cells.find((cell): cell is AutofillField => cell !== null) as AutofillField;
    if (values.length === 0) {
      const reasons = [...new Set(entry.cells.map((cell) => cell?.skip ?? '').filter(Boolean))];
      section(entry.form).skipped.push({ key, field: first, reasons });
      skipped += 1;
      continue;
    }
    const value = common(values);
    const cells = entry.cells.map((cell, at) => ({ view: filled[at] as AutofillView, field: cell, same: valueOf(cell) === value }));
    const field = cells.find((cell) => cell.same)?.field ?? first;
    section(entry.form).rows.push({ key, field, value, differs: cells.filter((cell) => !cell.same).length, cells });
    fields += 1;
  }
  return { sections: [...sections.values()], filled, others, fields, skipped };
}

/** A view's entry for a field in words: its value, why it was skipped, or that it is missing. */
export function viewText(t: Translate, cell: Cell): string {
  if (!cell.field) return t('panel.notOnPage');
  if (cell.field.skip) return t('panel.skipped', { reason: reasonText(t, cell.field.skip) });
  return cell.field.value;
}

/** Why a view was not filled, in words; `say` turns a failure code into its sentence. */
function viewState(t: Translate, say: (code: string) => string, view: AutofillView): string {
  if (view.status === 'unsupported') return t('panel.status.unsupported', { engine: ENGINES[view.engine] ?? view.engine });
  if (view.status === 'failed') return t('panel.status.failed', { error: say(view.error ?? '') });
  if (['empty', 'not-found', 'selector', 'timeout'].includes(view.status)) return t('panel.status.' + view.status);
  return view.status;
}

/** The status line of a fill: what was filled, then each view not filled, once. */
export function statusLine(t: Translate, say: (code: string) => string, result: AutofillResult, seen: Outcome): StatusLine {
  const summary = t('panel.summary', { fields: seen.fields, mode: t('panel.modes.' + result.mode), views: seen.filled.length, seed: result.seed });
  const notes = seen.others.map((view) => t('panel.viewNote', { view: view.label, state: viewState(t, say, view).replace(/\.$/, '') }));
  const failed = seen.others.some((view) => view.status !== 'unsupported');
  return { tone: failed ? 'warn' : 'ok', text: [summary, ...notes].join(' ') };
}

/** How the forms checked after the fill: valid everywhere, invalid everywhere, or in how many views. */
export function formState(t: Translate, filled: AutofillView[]): { text: string; color: 'success' | 'error' | 'warning' } | null {
  const checked = filled.filter((view) => typeof view.formValid === 'boolean');
  if (checked.length === 0) return null;
  const invalid = checked.filter((view) => view.formValid === false).length;
  if (invalid === 0) return { text: t('panel.formValid'), color: 'success' };
  if (invalid === checked.length) return { text: t('panel.formInvalid'), color: 'error' };
  return { text: t('panel.formMixed', { count: invalid }), color: 'warning' };
}
