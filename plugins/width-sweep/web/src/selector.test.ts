import { afterEach, describe, expect, it } from 'vitest';
import { cssPath, fullPath } from './selector';

afterEach(() => {
  document.body.innerHTML = '';
});

describe('selectors', () => {
  it('a short selector is an id, else tag and classes, made unique upwards', () => {
    document.body.innerHTML =
      '<main id="top"><ul><li class="item">a</li><li class="item">b</li></ul></main><p class="lead">x</p>';
    expect(cssPath(document.querySelector('main')!)).toBe('#top');
    expect(cssPath(document.querySelector('p')!)).toBe('p.lead');
    expect(cssPath(document.querySelectorAll('li')[1]!)).toBe('li.item:nth-of-type(2)');
  });

  it('a full path is the shared form of the findings: a step numbered only among siblings of its tag (RB-094-10)', () => {
    document.body.innerHTML = '<div></div><div><span></span><b></b><span class="x"></span></div>';
    const span = document.querySelector('span.x')!;
    expect(fullPath(span)).toBe('html > body > div:nth-of-type(2) > span:nth-of-type(2)');
    expect(fullPath(document.querySelector('b')!)).toBe('html > body > div:nth-of-type(2) > b');
    expect(fullPath(document.documentElement)).toBe('html');
  });
});
