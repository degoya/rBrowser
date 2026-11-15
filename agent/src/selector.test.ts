import { beforeEach, describe, expect, it } from 'vitest';
import {
  cssPath,
  describeSelector,
  isGeneratedClass,
  isGeneratedId,
  makeSelector,
  makeTarget,
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
