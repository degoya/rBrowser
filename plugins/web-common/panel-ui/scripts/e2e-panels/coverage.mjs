// The Coverage panel in the sandboxed frame (RB-080-84), loaded by e2e.mjs: a load measurement
// in two Chromium views and one Firefox view, one file opened to its marked source.
const SUMMARY = {
  mode: 'load',
  running: false,
  started: 1,
  finished: 2,
  files: [
    { id: 0, url: 'https://site.test/assets/site.css', kind: 'css', inline: false },
    { id: 1, url: 'https://site.test/src/main.scss', kind: 'js', inline: false },
    { id: 2, url: 'https://site.test/assets/app.js', kind: 'js', inline: false },
  ],
  views: [
    { view: 1, label: 'View 1', breakpoint: 'desktop', width: 1280, engine: 'chromium', url: '', status: 'measured', files: [{ file: 0, total: 9000, used: 6000, unusedPercent: 33.3 }, { file: 1, total: 4000, used: 1000, unusedPercent: 75 }, { file: 2, total: 52000, used: 30000, unusedPercent: 42.3 }] },
    { view: 2, label: 'View 2', breakpoint: 'tablet', width: 768, engine: 'chromium', url: '', status: 'measured', files: [{ file: 0, total: 9000, used: 2000, unusedPercent: 77.8 }, { file: 1, total: 4000, used: 1000, unusedPercent: 75 }, { file: 2, total: 52000, used: 30000, unusedPercent: 42.3 }] },
    { view: 3, label: 'View 3', breakpoint: 'mobile', width: 375, engine: 'firefox', url: '', status: 'unavailable', error: 'plugins.error.not_available', files: [] },
  ],
  truncated: false,
  findingCount: 3,
  findings: [],
};

const TEXT = 'body { margin: 0 }\n@media (min-width: 1024px) { .wide { display: grid } }\n';

export default {
  title: 'Coverage',
  tooltip: '#refresh',
  answer: (call) => {
    if (call.tool === 'get') return SUMMARY;
    if (call.tool === 'file') {
      return {
        url: call.args.url, kind: 'css', inline: false, total: TEXT.length, offset: 0, length: 8000, next: null,
        views: [{ view: 1, label: 'View 1', breakpoint: 'desktop', total: TEXT.length, used: 18, unusedPercent: 75, segments: [[0, 18]] }],
        text: TEXT,
        rules: [{ prelude: 'body', start: 0, end: 18, usedIn: [1, 2] }, { prelude: '.wide', start: 48, end: 71, usedIn: [] }],
      };
    }
    return {};
  },
  check: async (frame, expect) => {
    await frame.locator('#files tbody tr[data-selectable="true"]').first().waitFor({ timeout: 5000 });
    const names = await frame.locator('#files [data-name]').allTextContents();
    expect('coverage: one row per file, the largest lever first', names.join(',') === 'app.js,main.scss,site.css', names);
    await frame.locator('#files tbody tr[data-selectable="true"]').nth(2).click();
    await frame.locator('[data-views] [data-open]').click();
    await frame.locator('#source').waitFor({ timeout: 5000 });
    const unused = await frame.locator('#source .cov-unused').count();
    expect('coverage: the unused parts marked', unused > 0, unused);
    const coloured = await frame.locator('#source [class*="hl-"]').count();
    expect('coverage: the source coloured', coloured > 0, coloured);
  },
};
