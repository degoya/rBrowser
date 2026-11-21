// The srcset reader (RB-093-02): candidates, their descriptors and the one a URL is.
import { describe, expect, it } from 'vitest';
import { candidateOf, hasWidthDescriptors, parseSrcset } from './srcset';

describe('parseSrcset', () => {
  it('reads width and density descriptors and a candidate without one', () => {
    expect(parseSrcset('a.jpg 400w, b.jpg 800w')).toEqual([
      { url: 'a.jpg', width: 400 },
      { url: 'b.jpg', width: 800 },
    ]);
    expect(parseSrcset('a.jpg, b.jpg 2x,c.jpg 3.5x')).toEqual([
      { url: 'a.jpg' },
      { url: 'b.jpg', density: 2 },
      { url: 'c.jpg', density: 3.5 },
    ]);
  });

  it('keeps commas inside a URL and drops a candidate with a malformed descriptor', () => {
    expect(parseSrcset('img.php?size=1,2 1x , other.jpg 0w, x.jpg 2q, y.jpg 1x 2x')).toEqual([
      { url: 'img.php?size=1,2', density: 1 },
    ]);
  });

  it('reads nothing from an empty or blank attribute', () => {
    expect(parseSrcset('')).toEqual([]);
    expect(parseSrcset('  ,  ')).toEqual([]);
  });

  it('ends a candidate at a trailing comma of its URL, as the browser does', () => {
    expect(parseSrcset('a.jpg,, b.jpg 2x')).toEqual([{ url: 'a.jpg' }, { url: 'b.jpg', density: 2 }]);
    // A comma without whitespace after it belongs to the URL.
    expect(parseSrcset('a.jpg,b.jpg 2x')).toEqual([{ url: 'a.jpg,b.jpg', density: 2 }]);
  });
});

describe('hasWidthDescriptors', () => {
  it('is true when any candidate has a w descriptor', () => {
    expect(hasWidthDescriptors('a.jpg 1x, b.jpg 800w')).toBe(true);
    expect(hasWidthDescriptors('a.jpg 1x, b.jpg 2x')).toBe(false);
    expect(hasWidthDescriptors('')).toBe(false);
  });
});

describe('candidateOf', () => {
  it('finds the candidate a resolved URL is', () => {
    const base = 'https://example.test/page/';
    const found = candidateOf('a.jpg 400w, /img/b.jpg 800w', 'https://example.test/img/b.jpg', base);
    expect(found).toEqual({ url: '/img/b.jpg', width: 800 });
    expect(candidateOf('a.jpg 400w', 'https://example.test/other.jpg', base)).toBeNull();
  });
});
