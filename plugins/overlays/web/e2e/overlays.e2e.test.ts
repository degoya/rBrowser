// The built page script and measurement against tests/fixtures/site/overflow.html in a real
// headless Chromium (`npm run build && npm run test:e2e`). The page script runs in the page's
// main world here, with a fake `rbPlugin` that answers the hello with the state under test and
// records what the script sends and emits; in rBrowser it runs in the plugin's isolated world.

import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { type Browser, chromium, type Page } from 'playwright-core';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const pageScript = readFileSync(resolve(here, '../../page/overlays.js'), 'utf8');
const measureBundle = readFileSync(resolve(here, '../../assets/measure.js'), 'utf8');
const fixture = readFileSync(resolve(here, '../../../../tests/fixtures/site/overflow.html'), 'utf8');
const URL = 'http://fixture.test/overflow.html';

/** The expression `overlay_overflow` evaluates (src/report.rs `measure_script`). */
const measureScript = (show100vw: boolean): string =>
  `(() => {\n${measureBundle}\nreturn __rbOverlaysMeasure.measure({ show100vw: ${show100vw} });\n})()`;

interface Recorded {
  sent: { type: string; report?: { hasHorizontalScroll: boolean; causes: { selector: string; overhang: number }[] } }[];
  emitted: { type: string; count?: number }[];
}

let browser: Browser;
beforeAll(async () => {
  browser = await chromium.launch({ headless: true });
});
afterAll(async () => {
  await browser?.close();
});

/** The fixture at `width`, with `csp` as its header, without the page script yet. */
async function open(width: number, csp?: string): Promise<Page> {
  const context = await browser.newContext({ viewport: { width, height: 800 } });
  const page = await context.newPage();
  await page.route(URL, (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: fixture,
      headers: csp ? { 'content-security-policy': csp } : {},
    }),
  );
  await page.goto(URL);
  return page;
}

/** Runs the page script with a hello answering `all` as every view's state. */
async function inject(page: Page, all: Record<string, unknown>): Promise<void> {
  await page.evaluate((all) => {
    const recorded = { sent: [] as unknown[], emitted: [] as unknown[] };
    (window as unknown as { __recorded: unknown }).__recorded = recorded;
    (window as unknown as { rbPlugin: unknown }).rbPlugin = Object.freeze({
      id: 'overlays',
      view: 1,
      maxMessageBytes: 65536,
      send: (message: { type?: string }) => {
        recorded.sent.push(message);
        return Promise.resolve(message.type === 'hello' ? { state: { version: 1, all, views: {} }, settings: {} } : {});
      },
      emit: (message: unknown) => recorded.emitted.push(message),
      on: () => () => undefined,
      onStop: () => () => undefined,
    });
  }, all);
  // As the host injects it: a script evaluated over CDP, which no page CSP refuses.
  const cdp = await page.context().newCDPSession(page);
  const result = await cdp.send('Runtime.evaluate', { expression: pageScript });
  expect(result.exceptionDetails).toBeUndefined();
  await page.waitForTimeout(300);
}

const recorded = (page: Page): Promise<Recorded> =>
  page.evaluate(() => (window as unknown as { __recorded: Recorded }).__recorded);

/** `expression` evaluated over CDP, as the host's `evaluate` runs it; its value. */
async function evaluate(page: Page, expression: string): Promise<unknown> {
  const cdp = await page.context().newCDPSession(page);
  const result = await cdp.send('Runtime.evaluate', { expression, returnByValue: true });
  expect(result.exceptionDetails).toBeUndefined();
  return result.result.value;
}

const scrollWidth = (page: Page): Promise<number> => page.evaluate(() => document.documentElement.scrollWidth);

describe('the overflow detector', () => {
  it('at 375 px names exactly .promo, 41 px past the viewport', async () => {
    const page = await open(375);
    await inject(page, { overflow: { enabled: true } });
    const { sent } = await recorded(page);
    const reports = sent.filter((message) => message.type === 'overflow');
    expect(reports).toHaveLength(1);
    const report = reports[0]?.report;
    expect(report?.hasHorizontalScroll).toBe(true);
    expect(report?.causes.map((cause) => cause.selector)).toEqual(['div.promo']);
    expect(Math.abs((report?.causes[0]?.overhang ?? 0) - 41)).toBeLessThanOrEqual(1);
    await page.context().close();
  });

  it('at 1440 px reports and marks nothing', async () => {
    const page = await open(1440);
    await inject(page, { overflow: { enabled: true } });
    const { sent } = await recorded(page);
    expect(sent.filter((message) => message.type === 'overflow')).toEqual([]);
    const measured = await evaluate(page, measureScript(false));
    expect(measured).toMatchObject({ hasHorizontalScroll: false, causes: [] });
    await page.context().close();
  });

  it('measures for overlay_overflow as the component evaluates it, with the 100vw hint', async () => {
    const narrow = await open(375);
    const report = (await evaluate(narrow, measureScript(true))) as {
      hasHorizontalScroll: boolean;
      clientWidth: number;
      causes: { selector: string; overhang: number; rect: { width: number } }[];
      vw100: { selector: string }[];
    };
    expect(report.hasHorizontalScroll).toBe(true);
    expect(report.causes).toHaveLength(1);
    expect(report.causes[0]).toMatchObject({ selector: 'div.promo', overhang: 41 });
    expect(report.causes[0]?.rect.width).toBe(400);
    await narrow.context().close();
    const wide = await open(1440);
    const vw = (await evaluate(wide, measureScript(true))) as { hasHorizontalScroll: boolean; vw100: { selector: string }[] };
    expect(vw.hasHorizontalScroll).toBe(false);
    expect(vw.vw100.map((mark) => mark.selector)).toEqual(['section.hero']);
    await wide.context().close();
  });
});

describe('the overlay layer', () => {
  it('changes no scrollWidth with every overlay on, narrow and wide', async () => {
    for (const width of [375, 1440]) {
      const page = await open(width);
      const before = await scrollWidth(page);
      await inject(page, {
        overflow: { enabled: true },
        outlines: { enabled: true },
        highlight: { enabled: true, options: { selector: '.card' } },
      });
      expect(await page.evaluate(() => document.querySelector('rbrowser-overlays') !== null)).toBe(true);
      expect(await scrollWidth(page)).toBe(before);
      await page.context().close();
    }
  });

  it('counts the matches of .card', async () => {
    const page = await open(375);
    await inject(page, { highlight: { enabled: true, options: { selector: '.card', index: 1 } } });
    const { emitted } = await recorded(page);
    expect(emitted.filter((message) => message.type === 'matches').at(-1)).toMatchObject({ count: 3, index: 1 });
    await page.context().close();
  });

  it('is styled under a strict CSP (style-src none)', async () => {
    const page = await open(375, "default-src 'none'; style-src 'none'; script-src 'none'");
    await inject(page, { outlines: { enabled: true } });
    const position = await page.evaluate(() => {
      const host = document.querySelector('rbrowser-overlays');
      return host ? getComputedStyle(host).position : 'missing';
    });
    expect(position).toBe('fixed');
    await page.context().close();
  });
});
