// The page script in a real headless Chromium (RB-094-04's acceptance without the app):
// `npm run test:e2e`. Three pages stand for the views `mobile` (375 px), `md` (768 px) and `lg`
// (1280 px); the page script runs at document-start of every document, as the runtime injects it,
// with a stand-in `rbPlugin` whose answers come from the test and whose posts the test sends. The
// sheet is the component's for `body { outline: 3px solid red }` scoped to `md` (held to
// src/tools_tests.rs, `MD_SHEET`): the fixture reads the computed style in each view, before and
// after a navigation, on and off.

import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { type Browser, chromium, type Page } from 'playwright-core';
import { afterAll, beforeAll, beforeEach, describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const pageScript = readFileSync(resolve(here, '../../page/live-css.js'), 'utf8');
const MD_SHEET = '@media (min-width: 768px) and (max-width: 1279px) {\n  body { outline: 3px solid red }\n}';
const WIDTHS = [375, 768, 1280] as const;
const ORIGIN = 'http://example.test';

// The runtime's bridge as far as the page script uses it; `send` asks the test.
const STANDIN = `
(() => {
  const listeners = [];
  window.__post = (message) => listeners.forEach((listener) => listener(message));
  window.rbPlugin = Object.freeze({
    id: 'live-css', view: 1, maxMessageBytes: 65536,
    send: (message) => window.__answer(message),
    emit() {}, log() {},
    on(listener) { listeners.push(listener); return () => undefined; },
    onStop() { return () => undefined; },
  });
})();
`;

/** What the component answers a new document: the sheet, or nothing while the draft is off. */
let answer = MD_SHEET;
let browser: Browser;
let views: Page[] = [];

const html = (title: string): string =>
  `<!doctype html><html><head><title>${title}</title></head><body><h1>${title}</h1><a id="next" href="/next">Next</a></body></html>`;

async function open(width: number): Promise<Page> {
  const page = await browser.newPage({ viewport: { width, height: 600 }, deviceScaleFactor: 1 });
  await page.exposeFunction('__answer', () => ({ css: answer }));
  await page.route(`${ORIGIN}/**`, (route) =>
    route.fulfill({ contentType: 'text/html', body: html(new URL(route.request().url()).pathname) }),
  );
  await page.addInitScript({ content: `${STANDIN}\n(function (rbPlugin) {\n'use strict';\n${pageScript}\n})(window.rbPlugin);` });
  await page.goto(`${ORIGIN}/`);
  return page;
}

/** Each view's outline once its sheet arrived (`adopted` sheets expected). */
async function outlines(adopted: number): Promise<string[]> {
  return Promise.all(
    views.map(async (page) => {
      await page.waitForFunction((count) => document.adoptedStyleSheets.length === count, adopted);
      return page.evaluate(() => getComputedStyle(document.body).outlineStyle);
    }),
  );
}

async function postAll(message: unknown): Promise<void> {
  await Promise.all(views.map((page) => page.evaluate((posted) => (window as unknown as { __post(m: unknown): void }).__post(posted), message)));
}

async function navigateAll(): Promise<void> {
  await Promise.all(
    views.map(async (page) => {
      await Promise.all([page.waitForURL(`${ORIGIN}/next`), page.click('#next')]);
    }),
  );
}

beforeAll(async () => {
  browser = await chromium.launch({ headless: true });
});
afterAll(async () => {
  await browser?.close();
});
beforeEach(async () => {
  await Promise.all(views.map((page) => page.close()));
  answer = MD_SHEET;
  views = await Promise.all(WIDTHS.map(open));
});

describe('Live CSS in a real Chromium', () => {
  it('shows the outline scoped to md in the md view only, and again after a navigation', async () => {
    expect(await outlines(1)).toEqual(['none', 'solid', 'none']);
    await navigateAll();
    expect(views.map((page) => page.url())).toEqual(WIDTHS.map(() => `${ORIGIN}/next`));
    expect(await outlines(1)).toEqual(['none', 'solid', 'none']);
  });

  it('switched off, no view shows it, also after a navigation; switched on, md again', async () => {
    expect(await outlines(1)).toEqual(['none', 'solid', 'none']);
    await postAll({ css: '' });
    expect(await outlines(0)).toEqual(['none', 'none', 'none']);
    answer = '';
    await navigateAll();
    expect(await outlines(0)).toEqual(['none', 'none', 'none']);
    await postAll({ css: MD_SHEET });
    expect(await outlines(1)).toEqual(['none', 'solid', 'none']);
  });

  it('adds no element to the shared DOM', async () => {
    const page = views[1] as Page;
    await outlines(1);
    expect(await page.evaluate(() => document.querySelectorAll('style, link').length)).toBe(0);
  });

  it('applies from an isolated world, injected as the runtime injects it, and the page sees no script', async () => {
    const page = await browser.newPage({ viewport: { width: 768, height: 600 }, deviceScaleFactor: 1 });
    try {
      await page.route(`${ORIGIN}/**`, (route) =>
        route.fulfill({ contentType: 'text/html', body: html(new URL(route.request().url()).pathname) }),
      );
      // The bridge answers the start with the sheet; the page script runs in `rbplugin-live-css`
      // (Page.addScriptToEvaluateOnNewDocument with worldName, as crates/rbrowser injects it).
      const bridge = `const rbPlugin = Object.freeze({ send: () => Promise.resolve({ css: ${JSON.stringify(MD_SHEET)} }),
        log() {}, on() { return () => undefined; }, onStop() { return () => undefined; } });`;
      const cdp = await page.context().newCDPSession(page);
      await cdp.send('Page.enable');
      await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
        source: `(function () {\n'use strict';\n${bridge}\n${pageScript}\n})();`,
        worldName: 'rbplugin-live-css',
      });
      for (const path of ['/', '/next']) {
        await page.goto(`${ORIGIN}${path}`);
        await page.waitForFunction(() => document.adoptedStyleSheets.length === 1);
        expect(await page.evaluate(() => getComputedStyle(document.body).outlineStyle)).toBe('solid');
        expect(await page.evaluate(() => typeof (window as unknown as { rbPlugin?: unknown }).rbPlugin)).toBe('undefined');
      }
    } finally {
      await page.close();
    }
  });
});
