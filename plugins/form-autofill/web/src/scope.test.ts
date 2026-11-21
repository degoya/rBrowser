// RB-090-86: a fill without a selector fills the page's controls but those of its navigation -
// in a header, a nav, a menu or a search, and the disclosure toggles outside a form (a site's
// menu toggle) - whether they sit in a form or not; focus narrows nothing. `selector` names the
// element to fill. `apply` names to a recording only what it changed, so a second run on the
// same form adds no steps.
import { beforeEach, describe, expect, it } from 'vitest';
import { page } from './support';

// eslint-disable-next-line @typescript-eslint/no-explicit-any -- the page script's answer
type Answer = any;

const TOGGLE = '<input id="nav-toggle" type="checkbox" class="sr-only" aria-controls="main-nav" aria-label="Open the menu">';

const SITE = `<header>${TOGGLE}<nav id="main-nav"><a href="/">Home</a></nav><form role="search"><input name="site-q"></form></header>
<main>
  <form id="search" role="search"><input name="q"></form>
  <form id="contact">
    <select name="topic" required><option value="">-</option><option value="business">Business</option></select>
    <input name="first" required><input type="email" name="mail" required>
    <label><input type="checkbox" name="privacy" required> I agree</label>
    <label><input type="checkbox" name="other-address" aria-expanded="false" aria-controls="address"> Other address</label>
    <textarea name="message"></textarea>
  </form>
</main>
<footer><input name="loose"></footer>`;

const CONTACT = ['topic', 'first', 'mail', 'privacy', 'other-address', 'message'];

const names = (answer: Answer): string[] => answer.fields.map((field: Answer) => field.name || field.id);

beforeEach(() => {
  (document.activeElement as HTMLElement | null)?.blur?.();
});

describe('the fields a fill runs for', () => {
  it('are the page\'s but its navigation, search and menu toggle', () => {
    document.body.innerHTML = SITE;
    const answer = page({ op: 'collect' });
    expect(answer.scope).toBe('page');
    expect(names(answer)).toEqual([...CONTACT, 'loose']);
  });

  it('do not follow the focus: an autofocused search field narrows nothing', () => {
    document.body.innerHTML = SITE;
    (document.querySelector('input[name=site-q]') as HTMLInputElement).focus();
    expect(names(page({ op: 'collect' }))).toEqual([...CONTACT, 'loose']);
    (document.querySelector('input[name=first]') as HTMLInputElement).focus();
    expect(names(page({ op: 'collect' }))).toEqual([...CONTACT, 'loose']);
  });

  it('are the chosen element\'s, a search form included', () => {
    document.body.innerHTML = SITE;
    const answer = page({ op: 'collect', selector: '#search' });
    expect(answer.scope).toBe('selector');
    expect(names(answer)).toEqual(['q']);
  });

  it('on a page whose signup has no form are its fields, not the header\'s search form', () => {
    document.body.innerHTML = `<header><form role="search"><input name="q"></form>${TOGGLE}</header>
      <nav><input type="checkbox" name="submenu"></nav><div role="menu"><input type="checkbox" name="item"></div>
      <main><input name="name"><button aria-expanded="false" aria-controls="more">More</button>
      <input type="checkbox" name="details" aria-expanded="false" aria-controls="more"><textarea name="note"></textarea>
      <input type="checkbox" name="agree"><search><input name="find"></search></main>`;
    const answer = page({ op: 'collect' });
    expect(answer.scope).toBe('page');
    expect(names(answer)).toEqual(['name', 'note', 'agree']);
  });

  it('apply sets only the fields of the same scope as collect', () => {
    document.body.innerHTML = SITE;
    const toggle = document.getElementById('nav-toggle') as HTMLInputElement;
    const fields = page({ op: 'collect' }).fields as Answer[];
    const actions = fields.map((field) => field.type === 'checkbox'
      ? { index: field.index, sig: field.sig, op: 'check', checked: true }
      : field.tag === 'select'
        ? { index: field.index, sig: field.sig, op: 'select', values: ['business'], goal: 'valid' }
        : { index: field.index, sig: field.sig, op: 'text', candidates: ['x@example.com'], goal: 'any' });
    const answer = page({ op: 'apply', actions });
    expect(answer.fields.every((field: Answer) => field.status === 'done')).toBe(true);
    expect(toggle.checked).toBe(false);
    expect(JSON.stringify(answer.fields.map((field: Answer) => field.record))).not.toContain('nav-toggle');
  });
});

describe('what apply names to a recording', () => {
  const FORM = `<form id="f"><input name="a"><input type="checkbox" name="c" checked>
    <select name="s"><option value="">-</option><option value="x">X</option></select>
    <input type="radio" name="r" value="1" checked><input type="radio" name="r" value="2"></form>`;

  function actions(): Answer[] {
    const [a, c, s, r] = page({ op: 'collect' }).fields as Answer[];
    return [
      { index: a.index, sig: a.sig, op: 'text', candidates: ['hello'], goal: 'any' },
      { index: c.index, sig: c.sig, op: 'check', checked: true },
      { index: s.index, sig: s.sig, op: 'select', values: ['x'], goal: 'valid' },
      { index: r.index, sig: r.sig, op: 'radio', value: '1' },
    ];
  }

  it('is only what the run changed', () => {
    document.body.innerHTML = FORM;
    const answer = page({ op: 'apply', actions: actions() });
    const records = answer.fields.map((field: Answer) => field.record?.kind ?? null);
    // The text and the select changed; the checkbox and the radio already held what was asked.
    expect(records).toEqual(['fill', null, 'select', null]);
    expect(answer.fields.every((field: Answer) => field.status === 'done')).toBe(true);
  });

  it('is nothing when the same fill runs again on the same form', () => {
    document.body.innerHTML = FORM;
    const planned = actions();
    page({ op: 'apply', actions: planned });
    const again = page({ op: 'apply', actions: planned });
    expect(again.fields.map((field: Answer) => field.record)).toEqual([null, null, null, null]);
    expect(again.fields.every((field: Answer) => field.status === 'done')).toBe(true);
  });
});
