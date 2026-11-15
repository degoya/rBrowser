import { describe, expect, it } from 'vitest';

import { moveCell, pageLink, rating, rows, signed, type LighthouseSummary } from './panel/model';

const LATEST: LighthouseSummary = {
  runId: '20261112-110000',
  url: null,
  categories: ['performance', 'seo'],
  breakpoints: [
    { breakpoint: 'mobile', view: 1, width: 375, height: 812, formFactor: 'mobile', url: 'http://x.test/', scores: { performance: 48, seo: 92 }, report: '.rbrowser/out/lighthouse/20261112-110000/mobile.html', json: '', findings: 7 },
    { breakpoint: 'desktop', view: 2, width: 1440, height: 900, formFactor: 'desktop', url: 'http://x.test/', scores: { performance: null, seo: 75 }, report: '.rbrowser/out/lighthouse/20261112-110000/desktop.html', json: '', findings: 3 },
  ],
  previous: { runId: '20261112-100000', breakpoints: [{ breakpoint: 'mobile', scores: { performance: 52, seo: 92 } }] },
};

describe('the Lighthouse panel model', () => {
  it('rates scores as Lighthouse does', () => {
    expect([rating(90), rating(89), rating(50), rating(49), rating(null)]).toEqual(['good', 'average', 'average', 'poor', null]);
  });

  it("makes a row per breakpoint with the run's categories and the change since the last run", () => {
    const table = rows(LATEST);
    expect(table.categories).toEqual(['performance', 'seo']);
    expect(table.rows.map((row) => row.breakpoint)).toEqual(['mobile', 'desktop']);
    expect(table.rows[0]?.cells).toEqual([
      { category: 'performance', score: 48, rating: 'poor', delta: -4 },
      { category: 'seo', score: 92, rating: 'good', delta: null },
    ]);
    expect(table.rows[1]?.cells[0]).toEqual({ category: 'performance', score: null, rating: null, delta: null });
    expect(rows(null)).toEqual({ categories: [], rows: [] });
  });

  it('moves through the tiles with the arrow keys, Home and End', () => {
    expect(moveCell(0, 0, 'ArrowDown', 2, 3)).toEqual([1, 0]);
    expect(moveCell(1, 0, 'ArrowDown', 2, 3)).toEqual([1, 0]);
    expect(moveCell(1, 1, 'ArrowRight', 2, 3)).toEqual([1, 2]);
    expect(moveCell(1, 2, 'Home', 2, 3)).toEqual([1, 0]);
    expect(moveCell(1, 0, 'End', 2, 3)).toEqual([1, 2]);
    expect(moveCell(0, 0, 'Enter', 2, 3)).toBeNull();
  });

  it('opens only an http or https page', () => {
    expect(pageLink('https://x.test/a?b=1')).toBe('https://x.test/a?b=1');
    expect(pageLink('http://x.test')).toBe('http://x.test/');
    expect(pageLink('file:///C:/x.html')).toBeNull();
    expect(pageLink('javascript:alert(1)')).toBeNull();
    expect(pageLink('not a url')).toBeNull();
    expect(pageLink(null)).toBeNull();
  });

  it('signs a change', () => {
    expect([signed(3), signed(-4)]).toEqual(['+3', '-4']);
  });
});
