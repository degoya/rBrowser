// The page script in a real headless Chromium (RB-070-05), on tests/fixtures/site/data/ and on
// generated documents of 15 MB, 25 MB and invalid JSON. Opt-in, not part of `npm test`:
// `npm run build && npm run test:e2e` (Playwright's Chromium headless shell must be installed,
// `npx playwright-core install chromium-headless-shell`).
//
// The bundle runs in the page's main world here, at DOMContentLoaded (the manifest's
// `document-end`), behind a fake `rbPlugin` answering the default settings. `attachShadow` is
// opened for the test only, so the checks can look inside; in rBrowser the root is closed and
// the script runs in the plugin's isolated world.

import { chromium } from 'playwright-core';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const bundle = readFileSync(resolve(here, '../../page/viewer.js'), 'utf8');
const fixtures = resolve(here, '../../../../tests/fixtures/site/data');
const url = (path) => pathToFileURL(path).href;

const HARNESS = `
  (() => {
    const attach = Element.prototype.attachShadow;
    Element.prototype.attachShadow = function (init) { return attach.call(this, { ...init, mode: 'open' }); };
    const listeners = [];
    window.__copied = [];
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: (text) => { window.__copied.push(text); return Promise.resolve(); } },
    });
    window.__post = (message) => listeners.forEach((listener) => listener(message));
    const rbPlugin = Object.freeze({
      id: 'json-xml-viewer', view: 1, maxMessageBytes: 65536,
      send: () => Promise.resolve({ config: {} }),
      emit: () => undefined,
      on: (listener) => { listeners.push(listener); return () => undefined; },
      onStop: () => () => undefined,
    });
    document.addEventListener('DOMContentLoaded', () => {
      new Function('rbPlugin', ${JSON.stringify(`'use strict';\n${bundle}`)})(rbPlugin);
    });
  })();
`;

const failures = [];
function expect(what, ok, actual) {
  if (!ok) failures.push(`${what}: got ${JSON.stringify(actual)}`);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
}

const HOST = 'rbrowser-json-xml-viewer';
const rows = (page) =>
  page.evaluate((host) => {
    const root = document.querySelector(host)?.shadowRoot;
    return [...(root?.querySelectorAll('.row:not([hidden])') ?? [])].map((row) => row.textContent);
  }, HOST);
const text = (page, selector) =>
  page.evaluate(
    ([host, css]) => document.querySelector(host)?.shadowRoot?.querySelector(css)?.textContent ?? null,
    [HOST, selector],
  );
const shown = (page) =>
  page.waitForFunction((host) => !!document.querySelector(host)?.shadowRoot?.querySelector('.row, .banner, .error:not(:empty)'), HOST, { timeout: 10000 });

async function filter(page, query) {
  const input = page.locator(`${HOST} input`);
  await input.fill(query);
  await input.press('Enter');
}

const scratch = mkdtempSync(join(tmpdir(), 'rb-jsonxml-e2e-'));
const browser = await chromium.launch({ headless: true });
try {
  const context = await browser.newContext({ viewport: { width: 1000, height: 700 } });
  await context.addInitScript({ content: HARNESS });
  const page = await context.newPage();

  // user.json: the tree, the raw text on the command and back, JSONPath and "Copy path".
  await page.goto(url(join(fixtures, 'user.json')));
  await shown(page);
  const tree = await rows(page);
  expect('user.json shows as a tree', tree[0] === '{' && tree.includes('"name": "Ada Lovelace"'), tree.slice(0, 4));
  await page.evaluate(() => window.__post({ type: 'toggleRaw' }));
  const raw = await page.evaluate((host) => {
    const pre = document.querySelector(host).shadowRoot.querySelector('.raw');
    return pre.hidden ? null : pre.textContent;
  }, HOST);
  expect('the command shows the raw text', raw === readFileSync(join(fixtures, 'user.json'), 'utf8'), raw?.slice(0, 40));
  await page.evaluate(() => window.__post({ type: 'toggleRaw' }));
  expect('and the tree again', (await rows(page))[0] === '{', null);
  await filter(page, '$.items[*].id');
  const ids = await rows(page);
  expect('JSONPath $.items[*].id shows only the ids', ids.join('|') === '$.items[0].id101|$.items[1].id102|$.items[2].id103', ids);
  await page.locator(`${HOST} button`, { hasText: 'Copy path' }).click();
  const copied = await page.evaluate(() => window.__copied);
  expect('"Copy path" copies the path', copied[0] === '$.items[0].id', copied);
  await page.locator(`${HOST} [role="tree"]`).press('ArrowDown');
  expect('the arrows move in the tree', (await text(page, '.status')) === '$.items[1].id', await text(page, '.status'));

  // sitemap.xml: an XML tree, XPath without a namespace prefix.
  await page.goto(url(join(fixtures, 'sitemap.xml')));
  await shown(page);
  const sitemap = await rows(page);
  expect('sitemap.xml shows as an XML tree', sitemap[0]?.startsWith('<urlset'), sitemap.slice(0, 3));
  await filter(page, '//url/loc');
  const locs = await rows(page);
  expect('XPath //url/loc filters the URLs', locs.length === 3 && locs[0] === '/urlset/url[1]/loc<loc>https://shop.example.test/</loc>', locs);

  // feed.xml: Atom.
  await page.goto(url(join(fixtures, 'feed.xml')));
  await shown(page);
  const feed = await rows(page);
  expect('feed.xml shows as a tree', feed.some((row) => row.startsWith('<entry')), feed.slice(0, 4));

  // Invalid JSON: the raw text with the line and column.
  const invalid = join(scratch, 'invalid.json');
  writeFileSync(invalid, '{\n  "a": 1,\n}\n');
  await page.goto(url(invalid));
  await shown(page);
  const error = await text(page, '.error');
  expect('invalid JSON names line and column', error === 'Invalid JSON at line 3, column 1: unexpected character', error);

  // 15 MB within 2 s, virtual; 25 MB stays raw with a notice.
  const big = Array.from({ length: 120000 }, (_, i) => ({ id: i, name: `item ${i}`, tags: ['a', 'b'], text: 'x'.repeat(80) }));
  const large = join(scratch, 'large.json');
  writeFileSync(large, JSON.stringify(big));
  const size = readFileSync(large).length;
  await page.goto(url(large));
  await shown(page);
  const ready = await page.evaluate(() => performance.now());
  const loaded = await page.evaluate(() => performance.getEntriesByType('navigation')[0].domContentLoadedEventStart);
  console.log(`     (the engine's own load to DOMContentLoaded: ${Math.round(loaded)} ms; the viewer: ${Math.round(ready - loaded)} ms)`);
  const elements = await page.evaluate((host) => document.querySelector(host).shadowRoot.querySelectorAll('.row').length, HOST);
  expect(`${(size / 1048576).toFixed(1)} MB open in under 2 s (${Math.round(ready)} ms)`, size > 15e6 && ready < 2000, ready);
  expect(`only the rows in view exist (${elements})`, elements < 80, elements);
  const huge = join(scratch, 'huge.json');
  writeFileSync(huge, JSON.stringify([...big, ...big.slice(0, 80000)]));
  await page.goto(url(huge));
  await shown(page);
  const banner = await text(page, '.banner');
  expect('25 MB stays raw with a notice', banner?.includes('more than the viewer') && (await rows(page)).length === 0, banner);
} finally {
  await browser.close();
  rmSync(scratch, { recursive: true, force: true });
}
if (failures.length > 0) {
  console.error(failures.join('\n'));
  process.exit(1);
}
