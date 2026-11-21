// The measure of RB-093-02: each image's file against what its rendered size needs at the view's
// density, and the srcset, sizes and LCP problems - the acceptance fixture among them.
import { describe, expect, it } from 'vitest';
import { analyze, DEFAULTS, formatNumber } from './analyze';
import { BREAKPOINTS, fixtureImages, image } from './fixtures';

const rules = (images: ReturnType<typeof analyze>): string[] =>
  images.flatMap((one) => one.issues.map((issue) => `${one.selector} ${issue.rule}${issue.variant ? `.${issue.variant}` : ''}`));

describe('the acceptance fixture', () => {
  for (const breakpoint of BREAKPOINTS) {
    it(`has exactly the four problems in ${breakpoint.id} (DPR ${breakpoint.dpr}), each with the view's density`, () => {
      const measured = analyze(fixtureImages(breakpoint), { ...DEFAULTS, dpr: breakpoint.dpr });
      expect(rules(measured)).toEqual(['#hero lcp.lazy', '#large oversized', '#small undersized', '#no-sizes sizes-missing.img']);
      for (const issue of measured.flatMap((one) => one.issues)) expect(issue.params.dpr).toBe(String(breakpoint.dpr));
    });
  }
});

describe('the size against the density', () => {
  it('finds nothing for a correctly sized image, also at DPR 3', () => {
    for (const dpr of [1, 1.5, 2, 3]) {
      const [measured] = analyze([image('#ok', { width: 300, height: 200 }, dpr, 1)], { ...DEFAULTS, dpr });
      expect(measured?.issues).toEqual([]);
      expect(measured?.ratio).toBe(1);
      expect(measured?.needed).toEqual({ width: Math.round(300 * dpr), height: Math.round(200 * dpr) });
    }
  });

  it('says what the image needs and has', () => {
    const [measured] = analyze([image('#big', { width: 300, height: 200 }, 2, 2)], { ...DEFAULTS, dpr: 2 });
    expect(measured?.ratio).toBe(2);
    expect(measured?.issues).toEqual([
      {
        rule: 'oversized',
        params: { dpr: '2', rendered: '300 × 200', needed: '600 × 400', file: '1200 × 800', ratio: '2', factor: '1.5' },
      },
    ]);
  });

  it('takes the limits from the settings', () => {
    const big = image('#a', { width: 100, height: 100 }, 1, 1.4);
    expect(analyze([big], { ...DEFAULTS, dpr: 1 })[0]?.issues).toEqual([]);
    expect(analyze([big], { ...DEFAULTS, dpr: 1, oversizeFactor: 1.2 })[0]?.issues.map((one) => one.rule)).toEqual(['oversized']);
    const soft = image('#b', { width: 100, height: 100 }, 2, 0.75);
    expect(analyze([soft], { ...DEFAULTS, dpr: 2 })[0]?.issues.map((one) => one.rule)).toEqual(['undersized']);
    expect(analyze([soft], { ...DEFAULTS, dpr: 2, undersizeFactor: 0.7 })[0]?.issues).toEqual([]);
  });

  it('the same file is right at DPR 1 and too small at DPR 2', () => {
    const facts = image('#a', { width: 400, height: 300 }, 1, 1);
    expect(analyze([facts], { ...DEFAULTS, dpr: 1 })[0]?.issues).toEqual([]);
    expect(analyze([facts], { ...DEFAULTS, dpr: 2 })[0]?.issues.map((one) => one.rule)).toEqual(['undersized']);
  });

  it('allows a pixel of rounding', () => {
    const facts = image('#a', { width: 299.6, height: 199.6 }, 1, 1);
    facts.file = { width: 299, height: 199 };
    expect(analyze([facts], { ...DEFAULTS, dpr: 1 })[0]?.issues).toEqual([]);
  });

  it('measures the tighter axis (object-fit: cover crops the other)', () => {
    const facts = image('#a', { width: 400, height: 400 }, 1, 1);
    facts.file = { width: 800, height: 300 };
    const [measured] = analyze([facts], { ...DEFAULTS, dpr: 1 });
    expect(measured?.ratio).toBe(0.75);
    expect(measured?.issues.map((one) => one.rule)).toEqual(['undersized']);
  });

  it('does not measure a vector, an image not loaded, an invisible or a tiny one', () => {
    const vector = image('#svg', { width: 100, height: 100 }, 2, 0.1, { vector: true, file: null });
    const pending = image('#lazy', { width: 100, height: 100 }, 2, 0.1, { loaded: false, file: null });
    const hidden = image('#hidden', { width: 0, height: 0 }, 2, 1, { visible: false });
    const pixel = image('#pixel', { width: 1, height: 1 }, 3, 0.34);
    for (const measured of analyze([vector, pending, hidden, pixel], { ...DEFAULTS, dpr: 2 })) {
      expect(measured.issues).toEqual([]);
      expect(measured.ratio).toBeNull();
      expect(measured.needed).toBeNull();
    }
  });
});

describe('srcset and sizes', () => {
  it('finds widths without sizes on the img or on a source of its picture', () => {
    const own = image('#a', { width: 100, height: 100 }, 1, 1, { srcset: 'a.jpg 100w, b.jpg 200w' });
    const source = image('#b', { width: 100, height: 100 }, 1, 1, { picture: true, sourceWithoutSizes: true });
    const dense = image('#c', { width: 100, height: 100 }, 1, 1, { srcset: 'a.jpg 1x, b.jpg 2x' });
    const sized = image('#d', { width: 100, height: 100 }, 1, 1, { srcset: 'a.jpg 100w', sizes: '100px' });
    const issues = analyze([own, source, dense, sized], { ...DEFAULTS, dpr: 1 }).map((one) => one.issues);
    expect(issues[0]).toEqual([{ rule: 'sizes-missing', variant: 'img', params: { dpr: '1' } }]);
    expect(issues[1]).toEqual([{ rule: 'sizes-missing', variant: 'source', params: { dpr: '1' } }]);
    expect(issues[2]).toEqual([]);
    expect(issues[3]).toEqual([]);
  });

  it('takes a blank sizes for none', () => {
    const blank = image('#a', { width: 100, height: 100 }, 1, 1, { srcset: 'a.jpg 100w', sizes: '  ' });
    expect(analyze([blank], { ...DEFAULTS, dpr: 1 })[0]?.issues.map((one) => one.rule)).toEqual(['sizes-missing']);
  });
});

describe('the LCP image', () => {
  const lcp = (more: Parameters<typeof image>[4]) => image('#hero', { width: 800, height: 400 }, 1, 1, { lcp: true, ...more });

  it('should load eagerly and with high priority', () => {
    expect(analyze([lcp({ loading: 'lazy' })], { ...DEFAULTS, dpr: 1 })[0]?.issues).toEqual([{ rule: 'lcp', variant: 'lazy', params: { dpr: '1' } }]);
    expect(analyze([lcp({ loading: 'eager' })], { ...DEFAULTS, dpr: 1 })[0]?.issues).toEqual([{ rule: 'lcp', variant: 'priority', params: { dpr: '1' } }]);
    expect(analyze([lcp({ fetchpriority: 'high' })], { ...DEFAULTS, dpr: 1 })[0]?.issues).toEqual([]);
  });

  it('is not checked with checkLcp off, and lazy images that are no LCP are fine', () => {
    expect(analyze([lcp({ loading: 'lazy' })], { ...DEFAULTS, dpr: 1, checkLcp: false })[0]?.issues).toEqual([]);
    expect(analyze([image('#x', { width: 10, height: 10 }, 1, 1, { loading: 'lazy' })], { ...DEFAULTS, dpr: 1 })[0]?.issues).toEqual([]);
  });
});

describe('formatNumber', () => {
  it('keeps at most two decimals and drops trailing zeros', () => {
    expect(formatNumber(2)).toBe('2');
    expect(formatNumber(1.5)).toBe('1.5');
    expect(formatNumber(2.625)).toBe('2.63');
    expect(formatNumber(0.3333)).toBe('0.33');
  });
});
