// The panel's model (RB-080-84): one row per file with one value over the views, by unused bytes;
// the views not measured, once; the type by content; the used parts over the views and the source
// cut into used and unused pieces.
import { describe, expect, it } from 'vitest';
import { type CoverageSummary, bytes, contentKind, differs, filterRows, pieces, rows, shortName, totals, unmeasured, usedAnywhere, windowRanges } from './panel/model';

const SUMMARY: CoverageSummary = {
  mode: 'load',
  running: false,
  started: 1,
  finished: 2,
  files: [
    { id: 0, url: 'http://localhost:8080/coverage/site.css', kind: 'css', inline: false },
    { id: 1, url: 'http://localhost:8080/coverage/index.html#style-1', kind: 'css', inline: true },
    { id: 2, url: 'http://localhost:5173/src/main.scss?t=1', kind: 'js', inline: false },
    { id: 3, url: 'http://localhost:5173/src/app.js', kind: 'js', inline: false },
  ],
  views: [
    {
      view: 1, label: 'View 1', breakpoint: 'desktop', width: 1280, engine: 'chromium', url: '', status: 'measured',
      files: [{ file: 0, total: 74, used: 41, unusedPercent: 44.6 }, { file: 2, total: 4000, used: 1000, unusedPercent: 75 }, { file: 3, total: 100, used: 90, unusedPercent: 10 }],
    },
    {
      view: 2, label: 'View 2', breakpoint: 'mobile', width: 375, engine: 'chromium', url: '', status: 'measured',
      files: [{ file: 0, total: 74, used: 18, unusedPercent: 75.7 }, { file: 2, total: 4000, used: 1000, unusedPercent: 75 }],
    },
    { view: 3, label: 'Firefox', breakpoint: '', width: 1280, engine: 'firefox', url: '', status: 'unavailable', error: 'plugins.error.not_available', files: [] },
  ],
  truncated: false,
  findingCount: 0,
  findings: [],
};

describe('the coverage model', () => {
  it('builds one row per measured file, by unused bytes, with the least share and the span', () => {
    const all = rows(SUMMARY);
    expect(all.map((row) => row.name)).toEqual(['main.scss', 'site.css', 'app.js']);
    const [scss, css] = all;
    expect(scss).toMatchObject({ unused: 3000, total: 4000, percent: 75, maxPercent: 75, kind: 'css', measured: 'js' });
    expect(css).toMatchObject({ unused: 33, percent: 44.6, maxPercent: 75.7 });
    expect(css?.views.map((value) => value.name)).toEqual(['desktop', 'mobile']);
    expect(differs(css!)).toBe(true);
    expect(differs(scss!)).toBe(false);
    expect(rows(null)).toEqual([]);
  });

  it('names the views without a measurement once', () => {
    expect(unmeasured(SUMMARY)).toEqual([{ view: 3, name: 'Firefox', engine: 'firefox', status: 'unavailable', error: 'plugins.error.not_available' }]);
  });

  it('types a file by its content, not by how it came', () => {
    expect(contentKind('http://x.test/a.css', 'css')).toBe('css');
    expect(contentKind('http://x.test/src/main.scss?t=1', 'js')).toBe('css');
    expect(contentKind('http://x.test/src/App.vue?vue&type=style&index=0&lang.css', 'js')).toBe('css');
    expect(contentKind('http://x.test/src/App.vue?vue&type=script', 'js')).toBe('js');
    expect(contentKind('http://x.test/app.js', 'js')).toBe('js');
  });

  it('filters by type and address, and sums what it shows', () => {
    const all = rows(SUMMARY);
    expect(filterRows(all, 'css', '').map((row) => row.name)).toEqual(['main.scss', 'site.css']);
    expect(filterRows(all, 'all', 'APP').map((row) => row.name)).toEqual(['app.js']);
    expect(totals(filterRows(all, 'js', ''))).toEqual({ unused: 10, total: 100, percent: 10 });
    expect(bytes(512)).toBe('512 B');
    expect(bytes(3000)).toBe('2.9 KB');
  });

  it('merges the used segments of every view', () => {
    expect(usedAnywhere([{ segments: [[0, 10], [20, 30]] }, { segments: [[5, 25]] }, { segments: [] }])).toEqual([[0, 30]]);
    expect(windowRanges(100, 10, [[95, 102], [108, 120]])).toEqual([[0, 2], [8, 10]]);
  });

  it('cuts a window of the source into used and unused pieces', () => {
    const source = 'abcdefghij';
    expect(pieces(source, 100, [[102, 104], [108, 120]])).toEqual([
      { text: 'ab', used: false },
      { text: 'cd', used: true },
      { text: 'efgh', used: false },
      { text: 'ij', used: true },
    ]);
    expect(pieces(source, 0, [])).toEqual([{ text: source, used: false }]);
    expect(shortName('http://x.test/a/b.css?v=1')).toBe('b.css');
  });
});
