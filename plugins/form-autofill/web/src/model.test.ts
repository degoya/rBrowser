// The panel's logic without the DOM: the arguments of a fill and the answer of `fill` turned into
// a row per field (RB-080-86), with the plugin's real English texts.
import { describe, expect, it } from 'vitest';
import { format } from '../../../web-common/panel-ui/src/format';
import {
  type AutofillField,
  type AutofillResult,
  type AutofillView,
  fieldName,
  fillArgs,
  formState,
  formsOf,
  marks,
  newSeed,
  outcome,
  profiles,
  statusLine,
  viewText,
} from './panel/model';
import { pluginFile } from './support';

function flatten(value: unknown, prefix: string, out: Record<string, string>): Record<string, string> {
  if (typeof value === 'string') out[prefix] = value;
  else if (value && typeof value === 'object') {
    for (const [key, inner] of Object.entries(value)) flatten(inner, prefix ? `${prefix}.${key}` : key, out);
  }
  return out;
}
const STRINGS = flatten(JSON.parse(pluginFile('locales/en.json')), '', {});
const t = (key: string, params?: Record<string, unknown>): string => format(STRINGS[`plugin.form-autofill.${key}`] ?? key, params);
const say = (code: string): string => (code === 'plugins.error.host_failed' ? 'The view did not answer.' : code);

const field = (key: string, extra: Partial<AutofillField> = {}): AutofillField => ({
  key, label: key, kind: 'text', value: 'v', violation: null, profile: false, skip: '', status: 'done',
  valid: true, errors: [], ...extra,
});
const view = (id: number, label: string, fields: AutofillField[], extra: Partial<AutofillView> = {}): AutofillView => ({
  view: id, label, breakpoint: label, engine: 'chromium', status: 'filled', fields, ...extra,
});
const result = (views: AutofillView[], extra: Partial<AutofillResult> = {}): AutofillResult => ({
  mode: 'valid', seed: 706559, strategy: null, profile: null, views, ...extra,
});

describe('a fill', () => {
  it('sends only the arguments the controls set', () => {
    expect(fillArgs('valid', { strategy: 'bounds', selector: '  ', profile: '', seed: null })).toEqual({ mode: 'valid' });
    expect(fillArgs('invalid', { strategy: 'bounds', selector: ' #register ', profile: 'qa', seed: 42 }))
      .toEqual({ mode: 'invalid', strategy: 'bounds', selector: '#register', profile: 'qa', seed: 42 });
    expect(fillArgs('invalid', { strategy: 'loud', selector: '', profile: '', seed: -1 })).toEqual({ mode: 'invalid' });
    expect(fillArgs('valid', { strategy: '', selector: '', profile: '', seed: 99999999999 })).toEqual({ mode: 'valid' });
    expect(fillArgs('valid', { strategy: '', selector: 'x'.repeat(2000), profile: '', seed: 1.5 }))
      .toEqual({ mode: 'valid', selector: 'x'.repeat(1024) });
    expect(fillArgs('valid', { strategy: '', selector: '', profile: '', seed: 0 })).toEqual({ mode: 'valid', seed: 0 });
  });

  it('lists the profile names of the settings', () => {
    expect(profiles({ 'form-autofill.profiles': [{ name: 'qa' }, { values: {} }, 'x', { name: '' }] })).toEqual(['qa']);
    expect(profiles({})).toEqual([]);
  });

  it('draws a new seed the generated values can be repeated with', () => {
    expect(newSeed(() => 0)).toBe(1);
    expect(newSeed(() => 0.999999999)).toBe(999999);
  });
});

describe('the answer per field', () => {
  const answer = result([
    view(1, 'mobile', [
      field('first', { label: 'Vorname *', kind: 'givenName', value: 'Linda', required: true, form: 0 }),
      field('mail', { label: 'E-Mail', kind: 'email', value: 'linda@example.com', form: 0 }),
      field('menu', { label: 'Menu', kind: 'checkbox', value: 'true', form: null, skip: 'hidden' }),
      field('site', { label: 'Website', kind: 'url', value: '', skip: 'hidden', form: 0 }),
    ], { forms: [{ selector: '#register', name: 'Register' }] }),
    view(2, 'md', [
      field('first', { label: 'Vorname *', kind: 'givenName', value: 'Linda', required: true, form: 0 }),
      field('mail', { label: 'E-Mail', kind: 'email', value: 'linda@example.com', form: 0 }),
      field('menu', { label: 'Menu', kind: 'checkbox', value: 'true', form: null }),
      field('site', { label: 'Website', kind: 'url', value: '', skip: 'disabled', form: 0 }),
    ], { forms: [{ selector: '#register', name: 'Register' }] }),
    view(3, 'lg', [
      field('first', { label: 'Vorname *', kind: 'givenName', value: 'Linda', required: true, form: 0 }),
      field('mail', { label: 'E-Mail', kind: 'email', value: 'other@example.com', form: 0 }),
      field('menu', { label: 'Menu', kind: 'checkbox', value: 'true', form: null }),
    ], { forms: [{ selector: '#register', name: 'Register' }] }),
    view(4, 'firefox', [], { engine: 'firefox', status: 'unsupported' }),
  ]);
  const seen = outcome(answer, t);

  it('has a row per field with the value once, and counts the views it differs in', () => {
    expect(seen.filled.map((one) => one.label)).toEqual(['mobile', 'md', 'lg']);
    expect(seen.others.map((one) => one.label)).toEqual(['firefox']);
    const rows = seen.sections.flatMap((section) => section.rows);
    expect(rows.map((row) => [row.key, row.value, row.differs])).toEqual([
      ['first', 'Linda', 0],
      ['mail', 'linda@example.com', 1],
      ['menu', 'true', 1],
    ]);
    expect(seen.fields).toBe(3);
    const mail = rows[1];
    expect(mail?.cells.map((cell) => [cell.view.label, viewText(t, cell), cell.same])).toEqual([
      ['mobile', 'linda@example.com', true],
      ['md', 'linda@example.com', true],
      ['lg', 'other@example.com', false],
    ]);
    expect(rows[2]?.cells.map((cell) => viewText(t, cell))).toEqual(['skipped (hidden)', 'true', 'true']);
    expect(t('panel.differs', { count: 1 })).toBe('differs in 1 view');
    expect(t('panel.differs', { count: 2 })).toBe('differs in 2 views');
  });

  it('groups the fields skipped in every view, with their reasons', () => {
    const skipped = seen.sections.flatMap((section) => section.skipped);
    expect(skipped.map((row) => [row.key, row.reasons])).toEqual([['site', ['hidden', 'disabled']]]);
    expect(seen.skipped).toBe(1);
    // A field missing from a view says so beside the views that have it.
    const site = outcome(result([view(1, 'a', [field('x')]), view(2, 'b', [])]), t).sections[0]?.rows[0];
    expect(site?.cells.map((cell) => viewText(t, cell))).toEqual(['v', 'not on the page']);
    expect(site?.differs).toBe(1);
    expect(t('panel.skippedFields', { count: 6 })).toBe('6 fields skipped');
  });

  it('makes a section per form when the page has several, in the order of the fields', () => {
    expect(seen.sections.map((section) => [section.selector, section.name])).toEqual([
      ['#register', 'Register'],
      [null, 'Outside a form'],
    ]);
    const two = outcome(result([
      view(1, 'a', [field('a', { form: 1 }), field('b', { form: 0 })], { forms: [{ selector: '#one', name: '' }, { selector: 'form[name="b"]', name: 'b' }] }),
    ]), t);
    expect(two.sections.map((section) => [section.selector, section.name])).toEqual([
      ['form[name="b"]', 'b'],
      ['#one', 'Form 1'],
    ]);
    // An answer without forms (an older plugin) is one section.
    expect(outcome(result([view(1, 'a', [field('a')])]), t).sections).toHaveLength(1);
  });

  it('says a field name once, with the star of a required field set apart', () => {
    expect(fieldName(field('a', { label: 'Vorname *', required: true }))).toEqual({ text: 'Vorname', required: true });
    expect(fieldName(field('a', { label: 'Nachname*', required: true }))).toEqual({ text: 'Nachname', required: true });
    expect(fieldName(field('a', { label: 'Note *' }))).toEqual({ text: 'Note *', required: false });
  });

  it('lists the forms of every view once, for the choice of a form', () => {
    expect(formsOf(result([
      view(1, 'a', [], { forms: [{ selector: '#a', name: 'A' }] }),
      view(2, 'b', [], { forms: [{ selector: '#a', name: 'A' }, { selector: '#b', name: '' }] }),
      view(3, 'c', []),
    ]))).toEqual([{ selector: '#a', name: 'A' }, { selector: '#b', name: '' }]);
  });

  it('says in words what a value is', () => {
    expect(marks(t, field('a', { valid: false, violation: 'pattern', profile: true })).map((mark) => mark.text))
      .toEqual(['invalid', 'broken: pattern', 'from the profile']);
    expect(marks(t, field('a')).map((mark) => mark.text)).toEqual([]);
  });
});

describe('the status line', () => {
  it('counts the fields and views filled and names the views not filled once', () => {
    const answer = result([
      view(1, 'md', [field('a'), field('b'), field('c', { skip: 'hidden' })], { formValid: true }),
      view(2, 'lg', [field('a'), field('b')], { formValid: true }),
      view(3, 'mobile', [], { engine: 'firefox', status: 'unsupported' }),
    ]);
    expect(statusLine(t, say, answer, outcome(answer, t))).toEqual({
      tone: 'ok',
      text: 'Filled 2 fields with valid values in 2 views, seed 706559. mobile: Firefox, not filled.',
    });
    const one = result([view(1, 'md', [field('a')])], { mode: 'invalid', strategy: 'mixed', seed: 7 });
    expect(statusLine(t, say, one, outcome(one, t)).text).toBe('Filled 1 field with invalid values in 1 view, seed 7.');
  });

  it('warns about a view that failed, in words', () => {
    const answer = result([
      view(1, 'md', [field('a')]),
      view(2, 'lg', [], { status: 'failed', error: 'plugins.error.host_failed' }),
      view(3, 'xl', [], { status: 'not-found' }),
      view(4, 'sm', [], { status: 'empty' }),
      view(5, 'xs', [], { status: 'timeout' }),
    ]);
    expect(statusLine(t, say, answer, outcome(answer, t))).toEqual({
      tone: 'warn',
      text: 'Filled 1 field with valid values in 1 view, seed 706559. lg: failed: The view did not answer. '
        + 'xl: nothing matches the selector. sm: no fields. xs: not filled, the fill ran out of its time (Time for one fill).',
    });
  });

  it('says how the forms checked after the fill', () => {
    const checked = (...states: (boolean | null)[]) => states.map((formValid, at) => view(at, 'v' + at, [], { formValid }));
    expect(formState(t, checked(true, true))).toEqual({ text: 'Form valid', color: 'success' });
    expect(formState(t, checked(false, false, null))).toEqual({ text: 'Form invalid', color: 'error' });
    expect(formState(t, checked(true, false))).toEqual({ text: 'Form invalid in 1 view', color: 'warning' });
    expect(formState(t, checked(null))).toBeNull();
  });

  it('counts the cancelled submits in plurals', () => {
    expect(t('panel.blocked', { count: 1 })).toBe('1 submit cancelled');
    expect(t('panel.blocked', { count: 3 })).toBe('3 submits cancelled');
  });
});
