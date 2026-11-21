// The page clock and the animation rates (RB-094-07) in a real headless Chromium, on the fixture
// `tests/fixtures/site/clock.html` in two pages of different widths (two views). Opt-in, not
// part of `npm test`: `npm run test:clock` (Playwright's Chromium headless shell must be
// installed, `npx playwright-core install chromium-headless-shell`).
//
// It does over CDP what rBrowser does (`crates/rbrowser/src/time_control/`): the clock script
// `clock.js` with its configuration registered in the main world before the page's scripts and
// run at once (`script.rs::install_source`), removed and reset for the real time; the document
// timeline's rate (`Animation.setPlaybackRate`) per page, and the step `step.js` in an isolated
// world. Checked - the job's acceptance in Chromium: with the clock at 2026-12-24 10:00 (each
// page's local time) both pages report that date, also after a reload; after the reset the real
// time; with the animations at 0 the CSS and the Web Animation stand still between two readings,
// a step moves them, the rate holds after a reload, and at 100 % they move again.

import { chromium } from 'playwright-core';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const clock = readFileSync(resolve(root, 'crates/rbrowser/src/time_control/clock.js'), 'utf8');
const step = readFileSync(resolve(root, 'crates/rbrowser/src/time_control/step.js'), 'utf8');
const fixture = pathToFileURL(resolve(root, 'tests/fixtures/site/clock.html')).href;

const failures = [];
function expect(what, ok, actual) {
  if (!ok) failures.push(`${what}: got ${JSON.stringify(actual)}`);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
}

const pause = (ms) => new Promise((done) => setTimeout(done, ms));

/** `script.rs::install_source`: the clock, then its configuration as JSON. */
const installSource = (config) => `${clock}\n;globalThis[Symbol.for("rbrowser.clock")].set(${JSON.stringify(config)});`;
/** `script.rs::RESET_EXPRESSION`. */
const RESET = 'globalThis[Symbol.for("rbrowser.clock")]?.reset() ?? false';

/** What the fixture shows, read again after its next render. */
async function values(page) {
  await pause(300);
  return page.evaluate(() => window.__clock);
}

const pad = (value) => String(value).padStart(2, '0');
const localText = (date) =>
  `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;

const browser = await chromium.launch({ headless: true });
try {
  const context = await browser.newContext();
  const views = [];
  for (const width of [390, 1280]) {
    const page = await context.newPage();
    await page.setViewportSize({ width, height: 700 });
    await page.goto(fixture);
    const cdp = await context.newCDPSession(page);
    // A script for new documents runs only while the session has the Page domain on; in
    // rBrowser the sync agent's attach turns it on (`sync_agent::inject`).
    await cdp.send('Page.enable');
    views.push({ page, cdp, width });
  }

  // The clock, one configuration for every view (`model.rs::Clock::config`).
  const config = { local: [2026, 12, 24, 10, 0, 0, 0], offset: 0, anchor: Date.now(), speed: 0 };
  for (const view of views) {
    const { identifier } = await view.cdp.send('Page.addScriptToEvaluateOnNewDocument', {
      source: installSource(config),
      runImmediately: true,
    });
    view.script = identifier;
  }
  for (const view of views) {
    const shown = await values(view.page);
    expect(`${view.width} px: the clock reads 2026-12-24 10:00`, shown.local === '2026-12-24 10:00', shown.local);
    expect(`${view.width} px: Intl reads Dec 24, 2026`, shown.intl === 'Dec 24, 2026', shown.intl);
  }
  await views[0].page.reload();
  const reloaded = await values(views[0].page);
  expect('after a reload the clock still reads 2026-12-24 10:00', reloaded.local === '2026-12-24 10:00', reloaded.local);

  // Reset: the script goes, the document gets the real time back.
  for (const view of views) {
    await view.cdp.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: view.script });
    await view.cdp.send('Runtime.evaluate', { expression: RESET, returnByValue: true });
  }
  for (const view of views) {
    const before = localText(new Date());
    const shown = await values(view.page);
    const after = localText(new Date());
    expect(`${view.width} px: after the reset the real time`, [before, after].includes(shown.local), shown.local);
  }

  // Animations at 0: the CSS and the Web Animation stand still.
  for (const view of views) {
    await view.cdp.send('Animation.setPlaybackRate', { playbackRate: 0 });
  }
  for (const view of views) {
    const first = await values(view.page);
    await pause(500);
    const second = await values(view.page);
    expect(
      `${view.width} px: at 0 the CSS animation stands still`,
      first.cssTransform === second.cssTransform,
      [first.cssTransform, second.cssTransform],
    );
    expect(
      `${view.width} px: at 0 the Web Animation stands still`,
      first.webTransform === second.webTransform,
      [first.webTransform, second.webTransform],
    );
    // One step in rBrowser's isolated world (`script.rs::step_expression`).
    const { frameTree } = await view.cdp.send('Page.getFrameTree');
    const { executionContextId } = await view.cdp.send('Page.createIsolatedWorld', {
      frameId: frameTree.frame.id,
      worldName: 'rbrowser-time',
    });
    const { result } = await view.cdp.send('Runtime.evaluate', {
      expression: `(${step})(250)`,
      contextId: executionContextId,
      returnByValue: true,
    });
    const stepped = await values(view.page);
    expect(`${view.width} px: a step moves both animations`, result.value === 2, result.value);
    expect(
      `${view.width} px: after the step the CSS animation stands elsewhere`,
      stepped.cssTransform !== second.cssTransform,
      [second.cssTransform, stepped.cssTransform],
    );
    const still = await values(view.page);
    expect(
      `${view.width} px: and stands still there`,
      still.cssTransform === stepped.cssTransform,
      [stepped.cssTransform, still.cssTransform],
    );
  }

  // A new document has a timeline of its own: the session's rate holds there too.
  await views[1].page.reload();
  const fresh = await values(views[1].page);
  await pause(500);
  const freshLater = await values(views[1].page);
  expect(
    'after a reload the rate of 0 still holds',
    fresh.cssTransform === freshLater.cssTransform,
    [fresh.cssTransform, freshLater.cssTransform],
  );

  // Back to 100 %: they move again.
  for (const view of views) {
    await view.cdp.send('Animation.setPlaybackRate', { playbackRate: 1 });
  }
  for (const view of views) {
    const first = await values(view.page);
    await pause(500);
    const second = await values(view.page);
    expect(
      `${view.width} px: at 100 % the CSS animation moves again`,
      first.cssTransform !== second.cssTransform,
      [first.cssTransform, second.cssTransform],
    );
  }
} finally {
  await browser.close();
}
if (failures.length > 0) {
  console.error(failures.join('\n'));
  process.exit(1);
}
