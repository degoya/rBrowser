// The fixture form end to end, across the two halves of the plugin: the page script's `collect`
// of tests/fixtures/forms/register.html must be the committed register.collected.json, which the
// component's tests (tests/fixture.rs) plan into register.plan.<name>.json; here each plan is
// applied to a fresh copy of the form, as in every view. FORM_AUTOFILL_UPDATE=1 rewrites the
// collected fields.
import { describe, expect, it } from 'vitest';
import { loadForm, page, pluginFile, writePluginFile } from './support';

const COLLECTED = 'tests/fixtures/forms/register.collected.json';
const plan = (name: string) => JSON.parse(pluginFile(`tests/fixtures/forms/register.plan.${name}.json`));

function apply(name: string) {
  const log = loadForm();
  const planned = plan(name);
  const actions = planned.fields.map((field: any) => field.action).filter(Boolean);
  const answer = page({ op: 'apply', selector: '#register', skipHidden: true, actions });
  const form = document.getElementById('register') as HTMLFormElement;
  return { log, planned, answer, form, actions };
}

const value = (id: string) => (document.getElementById(id) as HTMLInputElement).value;

describe('the fixture form', () => {
  it('collects as committed', () => {
    loadForm();
    const fresh = `${JSON.stringify(page({ op: 'collect', selector: '#register', skipHidden: true }), null, 2)}\n`;
    if (process.env.FORM_AUTOFILL_UPDATE === '1') writePluginFile(COLLECTED, fresh);
    expect(fresh === pluginFile(COLLECTED)).toBe(true);
  });

  it('a valid fill makes the form valid and never submits', () => {
    const { log, answer, form, actions } = apply('valid-en');
    expect(form.checkValidity()).toBe(true);
    expect(answer.formValid).toBe(true);
    expect(answer.fields).toHaveLength(actions.length);
    expect(answer.fields.every((field: any) => field.status === 'done' && field.valid)).toBe(true);
    expect(log.submits()).toBe(0);
    expect(answer.blockedSubmits).toBe(0);
    expect(value('avatar')).toBe('');
    expect(value('promo')).toBe('');
    expect((document.getElementById('terms') as HTMLInputElement).checked).toBe(true);
    expect((document.querySelector('input[name=salutation]:checked') as HTMLInputElement | null)?.value).toBeTruthy();
    expect(value('email')).toMatch(/^[a-z0-9.]+@example\.com$/);
  });

  it('every filled text field gets an input and a change event', () => {
    const { log } = apply('valid-en');
    for (const id of ['firstName', 'email', 'zip', 'comment', 'country']) {
      expect(log.events.filter((event) => event.id === id).map((event) => event.type)).toEqual(['input', 'change']);
    }
  });

  it('a German view gets a German postal code and a +49 number', () => {
    apply('valid-de');
    expect(value('zip')).toMatch(/^[0-9]{5}$/);
    expect(value('phone')).toMatch(/^\+49 1[0-9]{2} [0-9]{8}$/);
    expect((document.getElementById('country') as HTMLSelectElement).value).toBe('DE');
  });

  it('two views with the same plan hold the same values', () => {
    const ids = ['firstName', 'lastName', 'email', 'username', 'password', 'zip', 'city', 'phone', 'age'];
    apply('valid-en');
    const first = ids.map(value);
    apply('valid-en');
    expect(ids.map(value)).toEqual(first);
  });

  it('an invalid fill breaks every field the browser checks', () => {
    const { answer, form, planned } = apply('invalid');
    expect(form.checkValidity()).toBe(false);
    expect(answer.formValid).toBe(false);
    for (const field of planned.fields) {
      if (!field.violation || field.violation === 'length' || field.violation === 'chars') continue;
      // happy-dom checks no date range; Chromium does (rangeOverflow): the value is one day out.
      if (field.key === 'input:date:birthday') {
        expect(value('birthday')).toBe('2008-01-02');
        continue;
      }
      const done = answer.fields.find((entry: any) => entry.index === field.action.index);
      expect(done?.valid, `${field.key} (${field.violation})`).toBe(false);
    }
  });

  it('a profile sets its e-mail address', () => {
    apply('profile');
    expect(value('email')).toBe('qa@example.com');
    expect(value('password')).not.toBe('hunter2');
  });

  it('a changed field is not touched', () => {
    const log = loadForm();
    const actions = plan('valid-en').fields.map((field: any) => field.action).filter(Boolean);
    document.getElementById('email')?.setAttribute('name', 'renamed');
    const answer = page({ op: 'apply', selector: '#register', skipHidden: true, actions });
    const email = actions.find((action: any) => action.sig.includes('|email|'));
    expect(answer.fields.find((field: any) => field.index === email.index)?.status).toBe('changed');
    expect(value('email')).toBe('');
    expect(log.events.some((event) => event.id === 'email')).toBe(false);
  });

  it('a submit while filling is cancelled and counted', () => {
    const log = loadForm();
    const form = document.getElementById('register') as HTMLFormElement;
    // A page that submits on change (its own script, as a form library may).
    form.addEventListener('change', () => form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })), { once: true });
    const actions = plan('valid-en').fields.map((field: any) => field.action).filter(Boolean);
    const answer = page({ op: 'apply', selector: '#register', skipHidden: true, actions });
    expect(answer.blockedSubmits).toBe(1);
    expect(log.submits()).toBe(0);
  });
});
