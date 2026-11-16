// The Web Vitals measurement of the built agent in a real headless Chromium (RB-060-10), on the
// fixture `tests/fixtures/site/vitals.html` at a narrow and a wide viewport. Opt-in, not part of
// `npm test`: `npm run build && npm run test:e2e` (Playwright's Chromium headless shell must be
// installed, `npx playwright-core install chromium-headless-shell`).
//
// The bundle runs in the page's main world here, the binding `__rbEmit` a Playwright binding;
// in rBrowser it runs in the isolated world. The observers are the same.

import { chromium } from 'playwright-core';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const bundle = readFileSync(resolve(here, '../dist/rb-agent.js'), 'utf8');
const fixture = pathToFileURL(resolve(here, '../../tests/fixtures/site/vitals.html')).href;

/** The last vitals report of a page at `width` x `height`, after a click on the slow button. */
async function measure(browser, width, height) {
  const context = await browser.newContext({ viewport: { width, height } });
  const page = await context.newPage();
  const reports = [];
  await page.exposeBinding('__rbEmit', (_source, payload) => {
    const message = JSON.parse(payload);
    if (message.kind === 'vitals') reports.push(message);
  });
  await page.addInitScript({ content: bundle });
  await page.goto(fixture);
  await page.waitForTimeout(2000);
  await page.click('#slow');
  await page.waitForTimeout(2500);
  await context.close();
  return reports.at(-1);
}

const failures = [];
function expect(what, ok, actual) {
  if (!ok) failures.push(`${what}: got ${JSON.stringify(actual)}`);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
}

const browser = await chromium.launch({ headless: true });
try {
  const narrow = await measure(browser, 375, 812);
  const wide = await measure(browser, 1280, 800);
  console.log(JSON.stringify({ narrow, wide }, null, 2));
  expect('narrow: CLS above 0.25 (poor)', narrow?.cls?.value > 0.25, narrow?.cls);
  expect('narrow: the shifted content is named', narrow?.cls?.sources?.length > 0, narrow?.cls);
  expect('wide: CLS 0', wide?.cls?.value === 0, wide?.cls);
  expect('narrow: LCP is the mobile hero', narrow?.lcp?.element === '#hero-mobile', narrow?.lcp);
  expect('wide: LCP is the desktop hero', wide?.lcp?.element === '#hero-desktop', wide?.lcp);
  expect('INP after the slow click above 500 ms (poor)', narrow?.inp?.value > 500, narrow?.inp);
  expect('INP names the button', narrow?.inp?.target === '#slow', narrow?.inp);
  // A file: URL has no response, so TTFB may be absent; FCP is always there.
  expect('FCP is measured', narrow?.fcp > 0, narrow?.fcp);
} finally {
  await browser.close();
}
if (failures.length > 0) {
  console.error(failures.join('\n'));
  process.exit(1);
}
