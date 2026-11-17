// The page script's `capture` (RB-090-70): which form it reads, the values a preset takes, and the
// fields that give none - passwords, secrets and file inputs never leave the page.
import { describe, expect, it } from 'vitest';
import { page, pluginFile } from './support';

type Case = { type?: string; name?: string; id?: string; autocomplete?: string; label?: string; placeholder?: string; reason: string };

/** `name="value"` when `value` is given, escaped for an attribute. */
const attr = (name: string, value: string | undefined): string =>
  value ? ` ${name}="${value.replace(/&/g, '&amp;').replace(/"/g, '&quot;')}"` : '';

// eslint-disable-next-line @typescript-eslint/no-explicit-any -- the page script's answer
const byName = (answer: any, name: string): any => answer.fields.find((field: any) => field.name === name || field.id === name);

describe('capture', () => {
  it('holds the secret rule to the component side (tests/fixtures/sensitive-fields.json)', () => {
    const cases = JSON.parse(pluginFile('tests/fixtures/sensitive-fields.json')) as Case[];
    for (const [at, entry] of cases.entries()) {
      const id = entry.id || `f${at}`;
      const input = `<input type="${entry.type ?? 'text'}"${attr('name', entry.name)} id="${id}"${attr('autocomplete', entry.autocomplete)}${attr('placeholder', entry.placeholder)} value="x">`;
      document.body.innerHTML = entry.label ? `<form><label for="${id}">${entry.label}</label>${input}</form>` : `<form>${input}</form>`;
      if (entry.type === 'checkbox') (document.getElementById(id) as HTMLInputElement).checked = true;
      const [field] = page({ op: 'capture', hiddenInputs: true }).fields;
      expect(field?.excluded ?? '', JSON.stringify(entry)).toBe(entry.reason);
      if (entry.reason) expect(field).not.toHaveProperty('value');
      else expect(field?.value, JSON.stringify(entry)).toBe(entry.type === 'checkbox' ? 'true' : 'x');
    }
  });

  it('reads the values a person typed, chose and ticked, but never a password or a file', () => {
    document.body.innerHTML = `<form id="signup" aria-label="Sign up">
      <input name="email" type="email"><input name="pw" type="password"><input name="avatar" type="file">
      <textarea name="note"></textarea><input type="checkbox" name="terms">
      <input type="radio" name="plan" value="free"><input type="radio" name="plan" value="pro">
      <select name="size"><option value="s">S</option><option value="m">M</option></select>
      <select name="tags" multiple><option value="a">A</option><option value="b">B</option></select>
    </form>`;
    const form = document.forms[0] as HTMLFormElement;
    (form.elements.namedItem('email') as HTMLInputElement).value = 'linda@example.com';
    (form.elements.namedItem('pw') as HTMLInputElement).value = 'hunter2';
    (form.elements.namedItem('note') as HTMLTextAreaElement).value = 'Two\nlines';
    (form.elements.namedItem('terms') as HTMLInputElement).checked = true;
    (form.querySelector('[value="pro"]') as HTMLInputElement).checked = true;
    (form.elements.namedItem('size') as HTMLSelectElement).value = 'm';
    for (const option of (form.elements.namedItem('tags') as HTMLSelectElement).options) option.selected = true;
    const answer = page({ op: 'capture' });
    expect(answer.scope).toBe('form');
    expect(answer.form).toEqual({ selector: '#signup', name: 'Sign up' });
    expect(byName(answer, 'email').value).toBe('linda@example.com');
    expect(byName(answer, 'note').value).toBe('Two\nlines');
    expect(byName(answer, 'terms').value).toBe('true');
    expect(byName(answer, 'plan').value).toBe('pro');
    expect(byName(answer, 'size').value).toBe('m');
    expect(byName(answer, 'tags').excluded).toBe('multiple');
    expect(byName(answer, 'pw')).toMatchObject({ excluded: 'password' });
    expect(byName(answer, 'avatar')).toMatchObject({ excluded: 'file' });
    expect(JSON.stringify(answer)).not.toContain('hunter2');
  });

  it("takes the focused field's form, else the only form, else every field; a selector wins", () => {
    document.body.innerHTML = `<form id="a"><input name="one"></form><form id="b"><input name="two"></form><input name="loose">`;
    expect(page({ op: 'capture' }).scope).toBe('document');
    expect(page({ op: 'capture' }).fields.map((field: { name: string }) => field.name)).toEqual(['one', 'two', 'loose']);
    (document.querySelector('[name="two"]') as HTMLInputElement).focus();
    const focused = page({ op: 'capture' });
    expect(focused.scope).toBe('focused');
    expect(focused.form.selector).toBe('#b');
    expect(focused.fields.map((field: { name: string }) => field.name)).toEqual(['two']);
    const chosen = page({ op: 'capture', selector: '#a' });
    expect(chosen.scope).toBe('selector');
    expect(chosen.fields.map((field: { name: string }) => field.name)).toEqual(['one']);
    expect(page({ op: 'capture', selector: '#none' }).error).toBe('not-found');
    expect(page({ op: 'capture', selector: '##' }).error).toBe('selector');
  });

  it('reads hidden inputs only when asked, and never one named like a token', () => {
    document.body.innerHTML = `<form><input type="hidden" name="campaign" value="spring">
      <input type="hidden" name="authenticity_token" value="t0k3n"><input name="email" value="a@b.c"></form>`;
    expect(page({ op: 'capture' }).fields.map((field: { name: string }) => field.name)).toEqual(['email']);
    const answer = page({ op: 'capture', hiddenInputs: true });
    expect(byName(answer, 'campaign')).toMatchObject({ type: 'hidden', value: 'spring', skip: '' });
    expect(byName(answer, 'authenticity_token')).toMatchObject({ excluded: 'secret' });
    expect(JSON.stringify(answer)).not.toContain('t0k3n');
  });

  it('names a value longer than a preset takes instead of sending it', () => {
    document.body.innerHTML = `<form><textarea name="essay"></textarea></form>`;
    (document.querySelector('textarea') as HTMLTextAreaElement).value = 'x'.repeat(1001);
    expect(byName(page({ op: 'capture' }), 'essay')).toEqual(expect.objectContaining({ excluded: 'long' }));
  });

  it('leaves the default fill unchanged and fills a hidden input only when asked', () => {
    document.body.innerHTML = `<form><input type="hidden" name="campaign" value=""><input name="email"></form>`;
    expect(page({ op: 'collect' }).fields).toHaveLength(1);
    const [hidden] = page({ op: 'collect', hiddenInputs: true }).fields;
    expect(hidden).toMatchObject({ type: 'hidden', skip: '' });
    page({ op: 'apply', hiddenInputs: true, actions: [{ index: hidden.index, sig: hidden.sig, op: 'text', candidates: ['spring'], goal: 'any' }] });
    expect((document.querySelector('[name="campaign"]') as HTMLInputElement).value).toBe('spring');
  });
});
