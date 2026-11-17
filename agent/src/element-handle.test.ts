import { beforeEach, describe, expect, it } from 'vitest';
import { findTarget, targetOf } from './element-handle';
import type { ElementTarget } from './protocol';

beforeEach(() => {
  document.body.innerHTML = `
    <nav><ul>
      <li data-testid="products"><a href="/p">Products</a><ul class="sub" style="display:none"><li>A</li></ul></li>
      <li id="about-item"><a href="/a">About</a></li>
    </ul></nav>`;
});

describe('targetOf', () => {
  it('names the element itself, not its actionable ancestor, by its centre', () => {
    const item = document.querySelector('[data-testid="products"]');
    const target = targetOf(item, window);
    expect(target?.element).toMatchObject({ kind: 'testid', testid: 'products' });
    expect(target?.offset).toEqual({ x: 0.5, y: 0.5 });
  });

  it('answers null for what is no element', () => {
    expect(targetOf(null, window)).toBeNull();
    expect(targetOf({ nodeType: 1 }, window)).toBeNull();
    expect(targetOf(document.createTextNode('x'), window)).toBeNull();
  });
});

describe('findTarget', () => {
  it('finds the element a target of another view names', () => {
    const item = document.querySelector('#about-item');
    const target = targetOf(item, window) as ElementTarget;
    expect(findTarget(target, document)).toBe(item);
  });

  it('finds a hidden element too: a submenu closed until the state is forced', () => {
    const hidden = document.querySelector('.sub > li');
    const target = targetOf(hidden, window) as ElementTarget;
    expect(findTarget(target, document)).toBe(hidden);
  });

  it('answers null for a missing element, a malformed target or a selector that throws', () => {
    const missing: ElementTarget = {
      element: { kind: 'testid', testid: 'missing', path: '#missing' },
      offset: { x: 0.5, y: 0.5 },
    };
    expect(findTarget(missing, document)).toBeNull();
    expect(findTarget({ element: 'nope' }, document)).toBeNull();
    expect(findTarget(undefined, document)).toBeNull();
    const broken: ElementTarget = {
      element: { kind: 'css', path: 'li[' },
      offset: { x: 0.5, y: 0.5 },
    };
    expect(findTarget(broken, document)).toBeNull();
  });
});
