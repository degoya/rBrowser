// Fields that appear or become enabled while a form is filled (RB-094-22), on the fixture
// tests/fixtures/forms/contact.html with its own script: the topic select renders the company and
// message fields asynchronously after its `change`, a checkbox enables the phone field, another
// the submit button. The passes are driven as src/fill.rs drives them: collect, apply the control
// fields first (with `settle` the page script waits after each), collect again, apply what is new.
import { describe, expect, it } from 'vitest';
import { page, pluginFile } from './support';

/** A quiet window short enough for a test; the setting's default is 300 ms. */
const SETTLE = { quiet: 60, cap: 2000, budget: 5000 };

// eslint-disable-next-line @typescript-eslint/no-explicit-any -- the fixture's own globals
const fixture = () => window as any;

function loadContact() {
  const html = pluginFile('tests/fixtures/forms/contact.html');
  document.head.innerHTML = '';
  document.body.innerHTML = (/<body>([\s\S]*)<\/body>/.exec(html)?.[1] ?? '').replace(/<script>[\s\S]*?<\/script>/g, '');
  // The page's own script, in a scope of its own so every test can run it again.
  (0, eval)(`(() => {\n${/<script>([\s\S]*?)<\/script>/.exec(html)?.[1] ?? ''}\n})()`);
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any -- the page script's fields
const byName = (fields: any[], name: string) => fields.find((field) => field.name === name);

// eslint-disable-next-line @typescript-eslint/no-explicit-any -- the page script's fields
function action(field: any, rest: Record<string, unknown>) {
  return { index: field.index, sig: field.sig, ...rest };
}

const value = (id: string) => (document.getElementById(id) as HTMLInputElement | null)?.value;

describe('a form whose choices bring fields', () => {
  it('has no fields from the select yet when the fill does not wait', () => {
    loadContact();
    const fields = page({ op: 'collect' }).fields;
    page({ op: 'apply', actions: [action(byName(fields, 'topic'), { op: 'select', values: ['sales'], goal: 'valid' })] });
    expect(byName(page({ op: 'collect' }).fields, 'company')).toBeUndefined();
  });

  it('fills the fields a choice brings in a second pass, after the page settled', async () => {
    loadContact();
    const first = page({ op: 'collect' }).fields;
    expect(byName(first, 'phone').skip).toBe('disabled');
    expect(first.every((field: { filled: boolean }) => !field.filled)).toBe(true);
    const answer = await page({ op: 'apply', settle: SETTLE, actions: [
      action(byName(first, 'topic'), { op: 'select', values: ['support'], goal: 'valid' }),
      action(byName(first, 'callback'), { op: 'check', checked: true }),
      action(byName(first, 'privacy'), { op: 'check', checked: true }),
      action(byName(first, 'name'), { op: 'text', candidates: ['Ada Lovelace'], goal: 'valid' }),
    ] });
    expect(answer.fields.map((field: { status: string }) => field.status)).toEqual(['done', 'done', 'done', 'done']);
    expect(answer.fields.every((field: { changed: boolean }) => field.changed)).toBe(true);

    // The second read: the select's fields are there and empty, the phone field is enabled, and
    // what the first pass set holds its value.
    const second = page({ op: 'collect' }).fields;
    for (const name of ['company', 'message', 'phone']) {
      expect(byName(second, name), name).toMatchObject({ skip: '', filled: false });
    }
    for (const name of ['name', 'topic', 'callback', 'privacy']) expect(byName(second, name).filled, name).toBe(true);
    const later = await page({ op: 'apply', settle: SETTLE, actions: ['company', 'message', 'phone'].map((name) =>
      action(byName(second, name), { op: 'text', candidates: [`${name} value`], goal: 'any' })) });
    expect(later.fields.map((field: { status: string }) => field.status)).toEqual(['done', 'done', 'done']);
    expect([value('company'), value('message'), value('phone'), value('name')])
      .toEqual(['company value', 'message value', 'phone value', 'Ada Lovelace']);
    expect(later.formValid).toBe(true);
  });

  it('enables the submit button through the privacy box but never clicks it or submits', async () => {
    loadContact();
    const fields = page({ op: 'collect' }).fields;
    const send = document.getElementById('send') as HTMLButtonElement;
    expect(send.disabled).toBe(true);
    const answer = await page({ op: 'apply', settle: SETTLE, actions: [
      action(byName(fields, 'privacy'), { op: 'check', checked: true }),
    ] });
    expect(send.disabled).toBe(false);
    expect(fixture().__clicks).toBe(0);
    expect(fixture().__submits).toBe(0);
    expect(answer.blockedSubmits).toBe(0);
  });
});
