import { beforeEach, describe, expect, it } from 'vitest';
import { cssPath, MAX_SELECTOR_CHARS } from './selector';

const $ = (selector: string): Element => {
  const found = document.querySelector(selector);
  if (!found) throw new Error(`no ${selector}`);
  return found;
};

beforeEach(() => {
  document.body.innerHTML = `
    <main id="content"><ul><li class="item">a</li><li class="item">b</li></ul></main>
    <div class="card wide extra">x</div>
    <section><p>one</p></section><section><p>two</p></section>`;
});

describe('cssPath', () => {
  it('takes a unique id', () => {
    expect(cssPath($('#content'))).toBe('#content');
  });

  it('stops as soon as the selector is unique, with at most two classes', () => {
    expect(cssPath($('.card'))).toBe('div.card.wide');
  });

  it('tells twins apart by position and climbs until the element is alone', () => {
    const second = document.querySelectorAll('li')[1] as Element;
    expect(cssPath(second)).toBe('li.item:nth-of-type(2)');
    const two = document.querySelectorAll('p')[1] as Element;
    const path = cssPath(two);
    expect(document.querySelectorAll(path)).toHaveLength(1);
    expect(document.querySelector(path)).toBe(two);
  });

  it('escapes what a selector cannot hold and stays within its bound', () => {
    document.body.insertAdjacentHTML('beforeend', `<b class="w-[50%] ${'x'.repeat(400)}">y</b>`);
    const path = cssPath($('b'));
    expect(path.length).toBeLessThanOrEqual(MAX_SELECTOR_CHARS);
    expect(path.startsWith('b.w-\\[50\\%\\]')).toBe(true);
  });
});
