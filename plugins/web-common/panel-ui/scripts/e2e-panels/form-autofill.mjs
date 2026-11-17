// The form autofill panel (RB-080-86) for e2e.mjs:
// `fill` answers two views on a page with two forms, one field differing between them and one
// skipped everywhere; the check fills and waits for a card per form and the per-field rows, then
// takes a form as a preset (RB-090-70) and waits for the inline dialog with its fields.
const field = (key, extra = {}) => ({ key, label: key, kind: 'text', value: 'v', violation: null, profile: false, skip: '', status: 'done', valid: true, errors: [], required: false, form: 0, ...extra });
const forms = [{ selector: '#login', name: 'Sign in' }, { selector: 'body > main:nth-child(2) > form:nth-child(3)', name: '' }];
const fillView = (id, label) => ({
  view: id, label, breakpoint: label, engine: 'chromium', status: 'filled', formValid: true, blockedSubmits: 0, forms,
  fields: [
    field('first', { label: 'Vorname *', kind: 'givenName', value: 'Linda', required: true }),
    field('mail', { label: 'E-Mail-Adresse *', kind: 'email', value: id === 1 ? 'linda.wilson54@example.com' : 'linda.w@example.com', required: true, form: 1 }),
    field('site', { label: 'Website', kind: 'url', value: '', skip: 'hidden', form: 1 }),
  ],
});

const capture = {
  view: 1, label: 'md', url: 'http://127.0.0.1:18767/forms/register.html', scope: 'focused', form: forms[0], name: 'Sign in',
  truncated: false, available: true, presets: [], profiles: [],
  fields: [
    { locator: 'first', label: 'Vorname *', kind: 'givenName', control: 'text', form: 0, selected: true, value: 'Linda' },
    { locator: 'pw', label: 'Password', kind: 'password', control: 'password', form: 0, selected: false, excluded: 'password' },
  ],
};

export default {
  title: 'Form autofill',
  tooltip: '#options',
  answer: (call) =>
    call.tool === 'fill'
      ? { mode: call.args.mode, seed: 706559, strategy: call.args.mode === 'invalid' ? 'mixed' : null, profile: null, views: [fillView(1, 'md'), fillView(2, 'lg'), { view: 3, label: 'mobile', breakpoint: 'mobile', engine: 'firefox', status: 'unsupported', fields: [] }] }
      : call.tool === 'capture' && !call.args?.pending
        ? capture
        : {},
  check: async (frame) => {
    await frame.locator('#fill-valid').click();
    await frame.locator('[data-form]').nth(1).waitFor({ timeout: 5000 });
    await frame.locator('[data-differs]').first().waitFor({ timeout: 5000 });
    await frame.locator('#take-form').click();
    await frame.locator('[data-capture-fields] li').first().waitFor({ timeout: 5000 });
    await frame.locator('[data-excluded-field]').first().waitFor({ timeout: 5000 });
  },
};
