import { beforeEach, describe, expect, it } from 'vitest';
import {
  cssPath,
  describeSelector,
  isGeneratedClass,
  isGeneratedId,
  makeSelector,
  makeTarget,
  matchAll,
} from './selector';
import { el, giveBoxes, loadPage, setViewport } from './test-page';

beforeEach(() => {
  setViewport(1440);
  loadPage();
  giveBoxes();
});

describe('makeSelector', () => {
  it('takes data-testid first', () => {
    const selector = makeSelector(el('[data-testid="save"]'));
    expect(selector).toMatchObject({ kind: 'testid', testid: 'save' });
    expect(describeSelector(selector)).toBe('[data-testid="save"]');
  });

  it('counts the duplicates of a testid', () => {
    const mobile = document.querySelectorAll('[data-testid="nav-pricing"]')[1];
    expect(mobile).toBeDefined();
    expect(makeSelector(mobile as Element)).toMatchObject({ kind: 'testid', nth: 1 });
  });

  it('takes role and accessible name for a button without testid', () => {
    const cancel = el('button.kGfCyH');
    const selector = makeSelector(cancel);
    expect(selector).toMatchObject({ kind: 'role', role: 'button', name: 'Cancel' });
    expect(describeSelector(selector)).toBe('role=button[name="Cancel"]');
  });

  it('names form fields by their label', () => {
    expect(makeSelector(el('#remember'))).toMatchObject({
      kind: 'role',
      role: 'checkbox',
      name: 'Remember me',
    });
    expect(makeSelector(el('#lang'))).toMatchObject({
      kind: 'role',
      role: 'combobox',
      name: 'Language',
    });
    expect(makeSelector(el('#notes'))).toMatchObject({ kind: 'role', role: 'textbox' });
  });

  it('takes the text of an element without role', () => {
    const selector = makeSelector(el('p > span'));
    expect(selector).toMatchObject({ kind: 'text', tag: 'span', text: 'Read more' });
  });

  it('falls back to a CSS path without generated classes', () => {
    const icon = el('li.item > i');
    const selector = makeSelector(icon);
    expect(selector.kind).toBe('css');
    expect(selector.path).not.toMatch(/css-|jsx-|sc-/);
    expect(document.querySelector(selector.path)).toBe(icon);
  });

  it('never puts a generated class into any path', () => {
    for (const element of document.querySelectorAll('body *')) {
      const { path } = makeSelector(element);
      expect(path).not.toMatch(/css-[a-z0-9]|jsx-\d|sc-[a-z]|kGfCyH/i);
      expect(document.querySelector(path)).toBe(element);
    }
  });
});

describe('visibleNth (RB-090-81)', () => {
  it('counts the cards a person sees: the fourth visible "Read more" is visibleNth 3', () => {
    document.body.innerHTML = [1, 2, 3, 4, 5]
      .map((n) => `<article><h2>Card ${n}</h2><a href="/c${n}">Read more</a></article>`)
      .join('');
    const fourth = document.querySelectorAll('a')[3] as Element;
    expect(makeSelector(fourth)).toMatchObject({
      kind: 'role',
      role: 'link',
      name: 'Read more',
      nth: 3,
      visibleNth: 3,
    });
  });

  it('skips a hidden twin: the desktop link after a hidden mobile menu is the first visible', () => {
    document.body.innerHTML =
      '<nav id="mobile" style="display:none"><a href="/trainees">Trainees</a>' +
      '<a href="/experienced">Experienced</a></nav>' +
      '<nav id="desktop"><a href="/trainees">Trainees</a><a href="/experienced">Experienced</a></nav>';
    const desktop = el('#desktop > a:nth-of-type(2)');
    expect(makeSelector(desktop)).toMatchObject({
      kind: 'role',
      role: 'link',
      name: 'Experienced',
      nth: 1,
      visibleNth: 0,
    });
  });

  it('leaves visibleNth out for the first match and for a hidden element', () => {
    document.body.innerHTML =
      '<a href="/a">More</a><a href="/b" style="display:none">More</a>';
    const [first, hidden] = Array.from(document.querySelectorAll('a'));
    expect(makeSelector(first as Element)).not.toHaveProperty('visibleNth');
    const selector = makeSelector(hidden as Element);
    expect(selector).toMatchObject({ kind: 'role', nth: 1 });
    expect(selector).not.toHaveProperty('visibleNth');
  });

  it('counts visible text matches too', () => {
    document.body.innerHTML =
      '<p><span style="display:none">Details</span></p><p><span>Details</span></p>' +
      '<p><span>Details</span></p>';
    const third = document.querySelectorAll('span')[2] as Element;
    expect(makeSelector(third)).toMatchObject({ kind: 'text', nth: 2, visibleNth: 1 });
  });
});

describe('within (RB-090-84)', () => {
  // A hover-only mega menu's link and a content link of the same name, both shown while the
  // menu is open: a view that has not opened the menu yet must still wait for the menu's link.
  const MENU =
    '<header><nav id="main-nav"><ul><li><a href="/news">News</a><div class="mega"><ul><li>' +
    '<a href="/news/brands">Brands &amp; Produkte</a></li></ul></div></li></ul></nav></header>' +
    '<main><p><a class="pill" href="/news/brands">Brands &amp; Produkte</a></p></main>';

  it('narrows a name the page shows twice to the nearest ancestor that makes it unique', () => {
    document.body.innerHTML = MENU;
    const menu = makeSelector(el('#main-nav .mega a'));
    expect(menu).toMatchObject({ kind: 'role', role: 'link', name: 'Brands & Produkte', within: '#main-nav' });
    expect(menu).not.toHaveProperty('nth');
    expect(menu).not.toHaveProperty('visibleNth');
    // The only `main` of the page scopes the content link.
    expect(makeSelector(el('a.pill'))).toMatchObject({ kind: 'role', within: 'main' });
    expect(describeSelector(menu)).toBe('#main-nav >> role=link[name="Brands & Produkte"]');
  });

  it('finds the element within its scope, and the whole page where the scope is missing', () => {
    document.body.innerHTML = MENU;
    const selector = makeSelector(el('#main-nav .mega a'));
    expect(matchAll(selector, document)).toEqual([el('#main-nav .mega a')]);
    expect(matchAll({ ...selector, within: '#no-such-nav' } as typeof selector, document)).toHaveLength(2);
  });

  it('keeps the role selector of a name shown twice without a scope, for the live sync', () => {
    document.body.innerHTML = '<div><a href="/a">Offers</a></div><div><a href="/b">Offers</a></div>';
    const [first, second] = Array.from(document.querySelectorAll('a'));
    const kept = makeSelector(first as Element);
    expect(kept).toMatchObject({ kind: 'role', role: 'link', name: 'Offers' });
    expect(kept).not.toHaveProperty('within');
    expect(kept).not.toHaveProperty('nth');
    // A later one keeps its visible index (RB-090-81).
    expect(makeSelector(second as Element)).toMatchObject({ kind: 'role', nth: 1, visibleNth: 1 });
  });

  it('keeps a hidden twin as RB-090-81 had it', () => {
    document.body.innerHTML = MENU.replace('<p>', '<p style="display:none">');
    expect(makeSelector(el('#main-nav .mega a'))).not.toHaveProperty('within');
  });
});

describe('cssPath', () => {
  it('stops at a stable unique id', () => {
    expect(cssPath(el('#login > button'))).toBe('#login > button');
  });

  it('numbers same-tag siblings', () => {
    const second = el('li.item');
    const path = cssPath(second);
    expect(document.querySelector(path)).toBe(second);
    expect(path).toMatch(/li(\.item|:nth-of-type\(2\))$/);
  });

  it('skips generated ids', () => {
    const button = el('[data-testid="save"]');
    button.id = ':r1:';
    expect(cssPath(button)).not.toContain(':r1:');
  });

  it('is relative to an open shadow root', () => {
    const host = el('#host');
    const root = host.attachShadow({ mode: 'open' });
    root.innerHTML = '<div><button>Inside</button></div>';
    const inside = root.querySelector('button') as Element;
    expect(cssPath(inside)).toBe('div > button');
  });
});

describe('generated names', () => {
  it.each(['css-1x2y3z', 'jsx-123456', 'sc-bdfBwQ', 'kGfCyH', 'Button_root__a1B2c', 'x1y2z3'])(
    'treats %s as generated',
    (name) => expect(isGeneratedClass(name)).toBe(true),
  );

  it.each(['card', 'list', 'item', 'nav-desktop', 'btn-primary', 'MuiButton-root'])(
    'keeps %s',
    (name) => expect(isGeneratedClass(name)).toBe(false),
  );

  it.each([':r1:', 'ember1234', 'a8f3e2c1-4b5d-4c6e-9f70-123456789abc'])(
    'treats the id %s as generated',
    (id) => expect(isGeneratedId(id)).toBe(true),
  );

  it.each(['login', 'main-nav', 'q'])('keeps the id %s', (id) =>
    expect(isGeneratedId(id)).toBe(false),
  );
});

describe('makeTarget', () => {
  it('measures the offset in the element and takes the actionable ancestor', () => {
    const save = el('[data-testid="save"]');
    save.innerHTML = '<b>Save</b>';
    const target = makeTarget(save.querySelector('b') as Element, 35, 30);
    expect(target.element).toMatchObject({ kind: 'testid', testid: 'save' });
    expect(target.offset).toEqual({ x: 0.25, y: 0.25 });
    expect(target.hosts).toBeUndefined();
  });

  it('clamps the offset into the element', () => {
    const target = makeTarget(el('#email'), -50, 500);
    expect(target.offset).toEqual({ x: 0, y: 1 });
  });

  it('lists the shadow hosts from the document inward', () => {
    const host = el('#host');
    const root = host.attachShadow({ mode: 'open' });
    root.innerHTML = '<button>Inside</button>';
    const target = makeTarget(root.querySelector('button') as Element, 60, 40);
    expect(target.hosts).toEqual([{ kind: 'css', path: '#host' }]);
    expect(target.element).toMatchObject({ kind: 'role', role: 'button', name: 'Inside' });
  });
});
