// The page script in a real headless Chromium (RB-060-04), on tests/fixtures/site/peek.html in
// two pages standing for two views (390 and 1280 px wide). Opt-in, not part of `npm test`:
// `npm run build && npm run test:e2e` (Playwright's Chromium headless shell must be installed,
// `npx playwright-core install chromium-headless-shell`).
//
// The bundle runs in the page's main world here, behind a fake `rbPlugin` whose requests reach
// this script through a Playwright binding; it answers like the component (a canned inspection,
// the highlight posted to the other page). In rBrowser it runs in the plugin's isolated world.

import { chromium } from 'playwright-core';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const bundle = readFileSync(resolve(here, '../../page/peek.js'), 'utf8');
const fixture = pathToFileURL(resolve(here, '../../../../tests/fixtures/site/peek.html')).href;

const CANNED = {
  tag: 'button', id: 'send', classes: ['btn'],
  size: { width: 1, height: 1, dpr: 1, deviceWidth: 1, deviceHeight: 1 },
  box: {
    margin: { top: 0, right: 0, bottom: 0, left: 0 }, border: { top: 0, right: 0, bottom: 0, left: 0 },
    padding: { top: 0, right: 0, bottom: 0, left: 0 }, content: { width: 1, height: 1 },
  },
  colors: { foreground: '#777777', background: '#ffffff', backgroundUncertain: false },
  contrast: { ratio: 4.48, level: 'aa_large', largeText: false, foreground: '#777777', background: '#ffffff' },
  font: { declared: 'x', rendered: null, size: 16, weight: 400, style: 'normal', lineHeight: 'normal', letterSpacing: 'normal' },
  role: 'button', name: 'Send', accessibility: 'page',
};

const FAKE_BRIDGE = (view) => `
  (() => {
    const listeners = [];
    window.__peekPost = (message) => listeners.forEach((listener) => listener(message));
    window.rbPlugin = Object.freeze({
      id: 'peek', view: ${view}, maxMessageBytes: 65536,
      send: (message) => window.__peekSend(JSON.stringify(message)).then((text) => JSON.parse(text)),
      emit: () => undefined,
      on: (listener) => { listeners.push(listener); return () => undefined; },
      onStop: () => () => undefined,
    });
  })();
`;

const pages = [];
const requests = [];

async function open(browser, view, width) {
  const context = await browser.newContext({ viewport: { width, height: 800 } });
  const page = await context.newPage();
  await page.exposeBinding('__peekSend', async (_source, text) => {
    const message = JSON.parse(text);
    requests.push({ view, message });
    if (message.type === 'config') return JSON.stringify({ config: {} });
    const post = (body) =>
      Promise.all(pages.map((other) => other.evaluate((m) => window.__peekPost(m), body)));
    if (message.type === 'inspect') {
      await post({ type: 'highlight', from: view, target: message.data.target });
      return JSON.stringify({ inspection: CANNED });
    }
    if (message.type === 'clear') await post({ type: 'clear', from: view });
    return '{}';
  });
  await page.addInitScript({ content: FAKE_BRIDGE(view) + bundle });
  await page.goto(fixture);
  pages.push(page);
  return page;
}

const failures = [];
function expect(what, ok, actual) {
  if (!ok) failures.push(`${what}: got ${JSON.stringify(actual)}`);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
}

const hostBox = (page) =>
  page.evaluate(() => {
    const host = document.querySelector('rbrowser-peek');
    return host ? host.isConnected : false;
  });

const browser = await chromium.launch({ headless: true });
try {
  const wide = await open(browser, 1, 1280);
  const narrow = await open(browser, 2, 390);
  const send = await wide.locator('#send').boundingBox();
  const link = await wide.locator('#link').boundingBox();

  // Without the modifier: nothing asked, nothing drawn, the click reaches the page.
  await wide.mouse.move(send.x + 5, send.y + 5);
  await wide.mouse.click(link.x + 5, link.y + 5);
  expect('no request without the modifier', !requests.some((r) => r.message.type === 'inspect'), requests);
  expect('no overlay without the modifier', !(await hostBox(wide)), null);
  expect('the click reached the page', (await wide.evaluate(() => document.body.dataset.linkClicked)) === '1', null);

  // Alt + hover: the inspection within 100 ms (the pointer rests on the link: Alt alone
  // inspects it, the move then the button).
  await wide.keyboard.down('Alt');
  const started = Date.now();
  await wide.mouse.move(send.x + 6, send.y + 6);
  await wide.waitForFunction(() => document.querySelector('rbrowser-peek')?.isConnected === true);
  const elapsed = Date.now() - started;
  expect(`the overlay within 100 ms (${elapsed} ms)`, elapsed < 100, elapsed);
  await wide.waitForTimeout(50);
  const inspect = requests.find((r) => r.message.type === 'inspect' && r.message.data.id === 'send')
    ?.message.data;
  expect('the button is read', inspect?.tag === 'button' && inspect.selector === '#send', inspect);
  expect('its colour is #777', inspect?.color === 'rgb(119, 119, 119)', inspect?.color);
  expect('the background resolves to the white body',
    inspect?.backgrounds.at(-1)?.color === 'rgb(255, 255, 255)', inspect?.backgrounds);
  expect('the box model is read', inspect?.box.padding.join() === '6,12,6,12', inspect?.box);
  expect('the heuristic role and name', inspect?.role === 'button' && inspect.name === 'Send', inspect);

  // The other view outlines the same element.
  expect('the narrow view outlines the button', await hostBox(narrow), null);

  // Alt + click locks and is kept from the page.
  await wide.mouse.move(link.x + 5, link.y + 5);
  await wide.mouse.click(link.x + 5, link.y + 5);
  await wide.keyboard.up('Alt');
  expect('the locked click did not reach the page',
    (await wide.evaluate(() => document.body.dataset.linkClicked)) === '1', null);
  expect('the locked tooltip stays after Alt is released', await hostBox(wide), null);
  await wide.keyboard.press('Escape');
  expect('Esc releases it', !(await hostBox(wide)), null);
  expect('and clears the other view', !(await hostBox(narrow)), null);

  // An oklch colour reaches the component as sRGB.
  await wide.keyboard.down('Alt');
  const oklch = await wide.locator('#oklch').boundingBox();
  await wide.mouse.move(oklch.x + 5, oklch.y + 5);
  await wide.waitForTimeout(100);
  await wide.keyboard.up('Alt');
  const last = requests.filter((r) => r.message.type === 'inspect').at(-1)?.message.data;
  expect('oklch is converted to rgb()', /^rgb\(/.test(last?.color ?? ''), last?.color);

  // AltGr typing (Ctrl+Alt) in an input leaves Peek asleep and the text as typed.
  const before = requests.length;
  await wide.focus('#altgr');
  await wide.keyboard.down('Control');
  await wide.keyboard.down('Alt');
  await wide.mouse.move(send.x + 7, send.y + 7);
  await wide.keyboard.up('Alt');
  await wide.keyboard.up('Control');
  await wide.keyboard.type('@{');
  expect('AltGr does not wake Peek',
    !requests.slice(before).some((r) => r.message.type === 'inspect'), requests.slice(before));
  expect('the input keeps its text', (await wide.inputValue('#altgr')) === '@{', null);
} finally {
  await browser.close();
}
if (failures.length > 0) {
  console.error(failures.join('\n'));
  process.exit(1);
}
