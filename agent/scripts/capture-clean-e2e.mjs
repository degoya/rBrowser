// The capture preparation (RB-080-12) in a real headless Chromium, on the fixture
// `tests/fixtures/site/noise/index.html` (a OneTrust-like banner that locks the scroll, a chat
// launcher, a CSS carousel, a spinner, a caret and a video). Opt-in, not part of `npm test`:
// `npm run test:capture-clean` (Playwright's Chromium headless shell must be installed,
// `npx playwright-core install chromium-headless-shell`).
//
// It does over CDP what rBrowser does (`crates/rbrowser/src/capture/clean.rs`): the timeline's
// rate to 0, `prefers-reduced-motion: reduce`, the page script `clean.js` in an isolated world
// of its own with the built-in list plus a project selector; the undo in reverse. Checked: two
// full-page shots 2 s apart are byte-identical with the preparation and differ without it, the
// banner, the chat and the project's element are gone while prepared, and afterwards the banner
// is back and the carousel runs again.

import { chromium } from 'playwright-core';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const source = readFileSync(resolve(root, 'crates/rb-visual/src/prepare/clean.js'), 'utf8');
const list = JSON.parse(readFileSync(resolve(root, 'crates/rb-visual/data/hide-selectors.json'), 'utf8'));
const fixture = pathToFileURL(resolve(root, 'tests/fixtures/site/noise/index.html')).href;
const selectors = [...list.groups.flatMap((group) => group.selectors), '#own-promo'];

const failures = [];
function expect(what, ok, actual) {
  if (!ok) failures.push(`${what}: got ${JSON.stringify(actual)}`);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
}

const pause = (ms) => new Promise((done) => setTimeout(done, ms));

/** rBrowser's preparation over `cdp`; what the undo needs. */
async function prepare(cdp) {
  const { playbackRate } = await cdp.send('Animation.getPlaybackRate');
  await cdp.send('Animation.setPlaybackRate', { playbackRate: 0 });
  await cdp.send('Emulation.setEmulatedMedia', {
    media: '',
    features: [{ name: 'prefers-reduced-motion', value: 'reduce' }],
  });
  const { frameTree } = await cdp.send('Page.getFrameTree');
  const { executionContextId } = await cdp.send('Page.createIsolatedWorld', {
    frameId: frameTree.frame.id,
    worldName: 'rbrowser-capture',
  });
  const options = { hide: true, selectors, freeze: true };
  const { result } = await cdp.send('Runtime.evaluate', {
    expression: `${source}\nglobalThis.__rbCaptureClean.prepare(${JSON.stringify(options)})`,
    contextId: executionContextId,
    returnByValue: true,
    awaitPromise: true,
  });
  await pause(100);
  return { rate: playbackRate, context: executionContextId, token: result.value.token };
}

async function undo(cdp, prepared) {
  await cdp.send('Runtime.evaluate', {
    expression: `${source}\nglobalThis.__rbCaptureClean.undo(${prepared.token})`,
    contextId: prepared.context,
    returnByValue: true,
  });
  await cdp.send('Emulation.setEmulatedMedia', { media: '', features: [] });
  await cdp.send('Animation.setPlaybackRate', { playbackRate: prepared.rate });
}

const shot = (page) => page.screenshot({ fullPage: true, animations: 'allow', caret: 'initial' });

/** What the page shows of the noise, read in the page. */
const state = (page) =>
  page.evaluate(() => {
    const shown = (id) => {
      const element = document.getElementById(id);
      return element !== null && getComputedStyle(element).display !== 'none';
    };
    const track = document.querySelector('.track');
    const animation = track?.getAnimations()[0];
    const video = document.getElementById('clip');
    return {
      banner: shown('onetrust-banner-sdk'),
      chat: shown('intercom-container'),
      promo: shown('own-promo'),
      carousel: animation ? { state: animation.playState, time: animation.currentTime } : null,
      video: video ? { paused: video.paused, time: video.currentTime } : null,
      scroll: getComputedStyle(document.documentElement).overflowY,
    };
  });

const browser = await chromium.launch({ headless: true });
try {
  const context = await browser.newContext({ viewport: { width: 800, height: 600 } });
  const page = await context.newPage();
  await page.goto(fixture);
  await page.waitForSelector('body[data-video-ready]', { timeout: 8000 }).catch(() => {
    console.log('note: no video in this Chromium (MediaRecorder); the video half is skipped');
  });
  await page.focus('#field');
  const cdp = await context.newCDPSession(page);

  // Without the preparation the two shots differ (the counter-check).
  const plainA = await shot(page);
  await pause(2000);
  const plainB = await shot(page);
  expect('without the preparation two shots 2 s apart differ', !plainA.equals(plainB), plainA.length);

  const before = await state(page);
  expect('the banner is shown at first', before.banner, before);
  expect('the carousel runs at first', before.carousel?.state === 'running', before.carousel);

  const first = await prepare(cdp);
  const during = await state(page);
  const cleanA = await shot(page);
  await undo(cdp, first);
  expect('prepared: the banner is hidden', !during.banner, during);
  expect('prepared: the chat widget is hidden', !during.chat, during);
  expect('prepared: the project selector hides its element', !during.promo, during);
  expect('prepared: the scroll lock is lifted', during.scroll !== 'hidden', during.scroll);
  expect('prepared: the carousel is paused', during.carousel?.state === 'paused', during.carousel);
  if (before.video) {
    expect('prepared: the video is paused on frame 0', during.video?.paused && during.video.time === 0, during.video);
  }

  await pause(2000);
  const second = await prepare(cdp);
  const cleanB = await shot(page);
  await undo(cdp, second);
  expect('with the preparation two shots 2 s apart are identical (diff 0 %)', cleanA.equals(cleanB), [
    cleanA.length,
    cleanB.length,
  ]);

  const after = await state(page);
  await pause(300);
  const later = await state(page);
  expect('afterwards the banner is shown again', after.banner && after.chat && after.promo, after);
  expect('afterwards the scroll lock is back', after.scroll === 'hidden', after.scroll);
  expect(
    'afterwards the carousel runs again',
    later.carousel?.state === 'running' && later.carousel.time > after.carousel.time,
    [after.carousel, later.carousel],
  );
  if (before.video && !before.video.paused) {
    expect('afterwards the video plays again', later.video && !later.video.paused, later.video);
  }
  const left = await page.evaluate(() => document.querySelectorAll('style[data-rbrowser-capture]').length);
  expect('afterwards no preparation style is left', left === 0, left);
} finally {
  await browser.close();
}
if (failures.length > 0) {
  console.error(failures.join('\n'));
  process.exit(1);
}
