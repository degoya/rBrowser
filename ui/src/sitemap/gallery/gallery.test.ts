import { describe, expect, it } from 'vitest';
import { galleryRun, RUN_ID } from './fixture';
import { ALL, labelOf, percentOf, problemsOf, runOfHash, shotOrder, shownBreakpoints, shownPages, step } from './gallery';

describe('the gallery route', () => {
  it('names a run with a plain id only', () => {
    expect(runOfHash(`#/sitemap-gallery?run=${RUN_ID}`)).toBe(RUN_ID);
    expect(runOfHash('#/sitemap-gallery')).toBeNull();
    for (const bad of ['../x', '.hidden', 'a/b', 'a b', 'x.', '', 'a%2Fb', 'a'.repeat(121)]) {
      expect(runOfHash(`#/sitemap-gallery?run=${encodeURIComponent(bad)}`), bad).toBeNull();
    }
  });
});

describe('the filters', () => {
  const run = galleryRun();

  it('show every breakpoint and page with All', () => {
    const filter = { breakpoint: ALL, status: 'all' as const };
    expect(shownBreakpoints(run, filter).map((bp) => bp.name)).toEqual(['mobile', 'desktop']);
    expect(shownPages(run, filter).map(({ index }) => index)).toEqual([0, 1, 2]);
  });

  it('keep one breakpoint\'s column', () => {
    expect(shownBreakpoints(run, { breakpoint: 'desktop', status: 'all' }).map(labelOf)).toEqual(['desktop']);
  });

  it('keep the pages without a problem or only those with one', () => {
    expect(shownPages(run, { breakpoint: ALL, status: 'ok' }).map(({ index }) => index)).toEqual([0]);
    expect(shownPages(run, { breakpoint: ALL, status: 'problems' }).map(({ index }) => index)).toEqual([1, 2]);
  });
});

describe('the problems of a page', () => {
  it('are a 404, a redirect and a failed shot', () => {
    const [fine, missing, old] = galleryRun().pages;
    expect(problemsOf(fine!)).toEqual([]);
    expect(problemsOf(missing!)).toEqual([{ kind: 'http', detail: '404' }]);
    expect(problemsOf(old!)).toEqual([
      { kind: 'redirect', detail: 'http://localhost:3000/new' },
      { kind: 'failed', detail: 'mobile: timeout' },
    ]);
  });
});

describe('the large view\'s order', () => {
  it('steps through the shown shots with an image, row by row', () => {
    const run = galleryRun();
    const order = shotOrder(run, { breakpoint: ALL, status: 'all' });
    expect(order).toEqual([
      { page: 0, breakpoint: 'mobile' },
      { page: 0, breakpoint: 'desktop' },
      { page: 1, breakpoint: 'mobile' },
      { page: 2, breakpoint: 'desktop' },
    ]);
    expect(step(order, order[0]!, 1)).toEqual(order[1]);
    expect(step(order, order[0]!, -1)).toBeNull();
    expect(step(order, order[3]!, 1)).toBeNull();
    const mobile = shotOrder(run, { breakpoint: 'mobile', status: 'problems' });
    expect(mobile).toEqual([{ page: 1, breakpoint: 'mobile' }]);
  });

  it('names a comparison in percent', () => {
    expect(percentOf(0.0123)).toBe('1.23');
  });
});
