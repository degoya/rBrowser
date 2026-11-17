import { beforeEach, describe, expect, it } from 'vitest';
import { elementPath } from './path';

describe('elementPath', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('names only tags when every step is the only child of its tag', () => {
    document.body.innerHTML = '<main><p>text</p></main>';
    const p = document.querySelector('p');
    expect(p && elementPath(p)).toBe('html > body > main > p');
  });

  it('counts siblings of the same tag only, 1-based', () => {
    document.body.innerHTML = '<main><img src="a"><p></p><img src="b"></main>';
    const second = document.querySelectorAll('img')[1];
    expect(second && elementPath(second)).toBe('html > body > main > img:nth-of-type(2)');
  });

  it('ignores ids and classes', () => {
    document.body.innerHTML = '<div id="x" class="a b"><span class="c"></span></div><div></div>';
    const span = document.querySelector('span');
    expect(span && elementPath(span)).toBe('html > body > div:nth-of-type(1) > span');
  });

  it('is `html` for the root', () => {
    expect(elementPath(document.documentElement)).toBe('html');
  });

  it('crosses a shadow root with " >>> "', () => {
    document.body.innerHTML = '<section></section><section></section>';
    const host = document.querySelectorAll('section')[1];
    if (!host) throw new Error('no host');
    const root = host.attachShadow({ mode: 'open' });
    root.innerHTML = '<button></button><button></button>';
    const button = root.querySelectorAll('button')[1];
    expect(button && elementPath(button)).toBe(
      'html > body > section:nth-of-type(2) >>> button:nth-of-type(2)',
    );
  });
});
