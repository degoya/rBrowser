// The page script's edges: the selector, hidden fields, caps, select multiple, a cancelled click.
import { describe, expect, it } from 'vitest';
import { page } from './support';

describe('collect', () => {
  it('reports a bad selector and a missing element', () => {
    document.body.innerHTML = '<form id="f"><input name="a"></form>';
    expect(page({ op: 'collect', selector: '##' }).error).toBe('selector');
    expect(page({ op: 'collect', selector: '#nothing' }).error).toBe('not-found');
    expect(page({ op: 'collect', selector: null }).fields).toHaveLength(1);
  });

  it('leaves out buttons and hidden inputs and gives repeated names their own keys', () => {
    document.body.innerHTML = `<form><input type="hidden" name="h"><input type="submit"><button>b</button>
      <input name="a"><input name="a"></form><form><input name="a"></form>`;
    const keys = page({ op: 'collect' }).fields.map((field: any) => field.key);
    expect(keys).toEqual(['input:text:a', 'input:text:a:1', 'input:text:a:2']);
  });

  it('skips hidden fields only while skipHidden is on, and disabled and read-only ones always', () => {
    document.body.innerHTML = `<input name="gone" style="display:none"><input name="off" disabled>
      <input name="ro" readonly>`;
    expect(page({ op: 'collect', skipHidden: true }).fields.map((f: any) => f.skip)).toEqual(['hidden', 'disabled', 'readonly']);
    expect(page({ op: 'collect', skipHidden: false }).fields.map((f: any) => f.skip)).toEqual(['', 'disabled', 'readonly']);
  });

  it('names every form of the document with a selector that finds it alone, and each field its form', () => {
    document.body.innerHTML = `<form id="login" aria-label="Sign in"><input name="u"></form>
      <div><form name="news"><input name="e"></form><form><input name="x"></form></div>
      <section id="side"><p></p><form title="Search"></form></section><input name="loose">`;
    const answer = page({ op: 'collect', selector: '#login' });
    expect(answer.forms.map((form: any) => form.name)).toEqual(['Sign in', 'news', '', 'Search']);
    for (const [at, form] of answer.forms.entries()) {
      expect(document.querySelectorAll(form.selector), form.selector).toHaveLength(1);
      expect(document.querySelector(form.selector)).toBe(document.forms[at]);
    }
    expect(answer.forms[0].selector).toBe('#login');
    expect(answer.forms[1].selector).toBe('form[name="news"]');
    expect(answer.fields.map((field: any) => field.form)).toEqual([0]);
    expect(page({ op: 'collect' }).fields.map((field: any) => field.form)).toEqual([0, 1, 2, null]);
    expect(page({ op: 'collect', selector: '#nothing' }).forms).toHaveLength(4);
  });

  it('stops at 300 fields', () => {
    document.body.innerHTML = '<input>'.repeat(320);
    const answer = page({ op: 'collect' });
    expect(answer.fields).toHaveLength(300);
    expect(answer.truncated).toBe(true);
  });
});

describe('apply', () => {
  it('selects several options of a multiple select and clears one with no values', () => {
    document.body.innerHTML = `<select name="m" multiple><option value="a">A</option><option value="b">B</option>
      <option value="c">C</option></select><select name="s"><option value="">-</option><option value="x">X</option></select>`;
    const [multi, single] = page({ op: 'collect' }).fields;
    const answer = page({ op: 'apply', actions: [
      { index: multi.index, sig: multi.sig, op: 'select', values: ['a', 'c'], goal: 'valid' },
      { index: single.index, sig: single.sig, op: 'select', values: [], goal: 'invalid' },
    ] });
    expect(answer.fields.map((field: any) => field.value)).toEqual(['a, c', '']);
  });

  it('keeps the first candidate when none is valid and the last when none is invalid', () => {
    document.body.innerHTML = '<input name="z" pattern="[0-9]{2}"><input name="y" pattern="[a-z]+">';
    const [z, y] = page({ op: 'collect' }).fields;
    const answer = page({ op: 'apply', actions: [
      { index: z.index, sig: z.sig, op: 'text', candidates: ['ab', 'cd'], goal: 'valid' },
      { index: y.index, sig: y.sig, op: 'text', candidates: ['ab', 'cd'], goal: 'invalid' },
    ] });
    expect(answer.fields.map((field: any) => field.value)).toEqual(['ab', 'cd']);
  });

  it('sets a checkbox whose click the page cancels', () => {
    document.body.innerHTML = '<input type="checkbox" name="c">';
    const box = document.querySelector('input') as HTMLInputElement;
    box.addEventListener('click', (event) => event.preventDefault());
    const [field] = page({ op: 'collect' }).fields;
    page({ op: 'apply', actions: [{ index: field.index, sig: field.sig, op: 'check', checked: true }] });
    expect(box.checked).toBe(true);
  });

  it('unchecks a radio group for an empty required one', () => {
    document.body.innerHTML = '<input type="radio" name="r" value="a" required checked><input type="radio" name="r" value="b">';
    const [group] = page({ op: 'collect' }).fields;
    const answer = page({ op: 'apply', actions: [{ index: group.index, sig: group.sig, op: 'radio', value: null }] });
    expect(answer.fields[0].value).toBe('');
    expect(answer.fields[0].errors).toContain('valueMissing');
  });

  it('reports a field that is gone', () => {
    document.body.innerHTML = '';
    const answer = page({ op: 'apply', actions: [{ index: 4, sig: 'x', op: 'text', candidates: ['a'], goal: 'any' }] });
    expect(answer.fields[0].status).toBe('missing');
    expect(answer.formValid).toBe(null);
  });
});

describe('passes (RB-094-22)', () => {
  it('names the fields that hold a value and skips an aria-disabled one', () => {
    document.body.innerHTML = `<input name="t" value="held"><input name="e"><input type="checkbox" name="c" checked>
      <select name="s"><option value="a">A</option><option value="b">B</option></select>
      <select name="p"><option value="a">A</option><option value="b" selected>B</option></select>
      <input name="off" aria-disabled="true">`;
    const fields = page({ op: 'collect' }).fields;
    expect(fields.map((field: any) => field.filled)).toEqual([true, false, true, false, true, false]);
    expect(fields.map((field: any) => field.skip)).toEqual(['', '', '', '', '', 'disabled']);
  });

  it('reports a field the page replaced while it settled as missing, never sets the old one', async () => {
    document.body.innerHTML = '<select name="s"><option value="">-</option><option value="x">X</option></select><div id="d"><input name="t"></div>';
    const select = document.querySelector('select') as HTMLSelectElement;
    select.addEventListener('change', () => setTimeout(() => {
      (document.getElementById('d') as HTMLElement).innerHTML = '<input name="t">';
    }, 10));
    const [s, t] = page({ op: 'collect' }).fields;
    const answer = await page({ op: 'apply', settle: { quiet: 40, cap: 2000, budget: 5000 }, actions: [
      { index: s.index, sig: s.sig, op: 'select', values: ['x'], goal: 'valid' },
      { index: t.index, sig: t.sig, op: 'text', candidates: ['a'], goal: 'any' },
    ] });
    expect(answer.fields.map((field: any) => field.status)).toEqual(['done', 'missing']);
    expect((document.querySelector('input[name=t]') as HTMLInputElement).value).toBe('');
  });

  it('waits no longer than its cap while the page keeps changing', async () => {
    document.body.innerHTML = '<input type="checkbox" name="c"><p id="tick"></p>';
    const tick = setInterval(() => {
      (document.getElementById('tick') as HTMLElement).textContent = String(Math.random());
    }, 5);
    try {
      const [c] = page({ op: 'collect' }).fields;
      const answer = await page({ op: 'apply', settle: { quiet: 50, cap: 120, budget: 1000 }, actions: [
        { index: c.index, sig: c.sig, op: 'check', checked: true },
      ] });
      expect(answer.fields[0]).toMatchObject({ status: 'done', changed: true });
    } finally {
      clearInterval(tick);
    }
  });

  it('gives each field an identity that a field inserted before it does not shift', () => {
    document.body.innerHTML = '<form><input class="a"><input class="b"></form>';
    const [a, b] = page({ op: 'collect' }).fields;
    expect(a.uid).not.toBe(b.uid);
    document.querySelector('form')?.insertAdjacentHTML('afterbegin', '<input class="new">');
    const after = page({ op: 'collect' }).fields;
    expect(after[1].key).toBe(a.key.replace(/0$/, '1'));
    expect(after.slice(1).map((field: any) => field.uid)).toEqual([a.uid, b.uid]);
    expect([a.uid, b.uid]).not.toContain(after[0].uid);
  });

  it('answers at once without a settle request, as before', () => {
    document.body.innerHTML = '<input type="checkbox" name="c">';
    const [c] = page({ op: 'collect' }).fields;
    const answer = page({ op: 'apply', actions: [{ index: c.index, sig: c.sig, op: 'check', checked: true }] });
    expect(answer.fields[0].status).toBe('done');
  });
});
