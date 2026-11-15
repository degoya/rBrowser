// The tab order overlay (RB-070-12) against tests/fixtures/site/tab-order/index.html in a real
// headless Chromium (`npm run build && npm run test:e2e`): the order the component measures
// (`__rbOverlaysMeasure.tabOrder`, as src/tab_order.rs evaluates it) against the order Tab really
// takes, the warnings in a mobile and a desktop view, and the page script drawing the markers and
// leaving no trace when switched off.

import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { type Browser, chromium, type Page } from 'playwright-core';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const pageScript = readFileSync(resolve(here, '../../page/overlays.js'), 'utf8');
const measureBundle = readFileSync(resolve(here, '../../assets/measure.js'), 'utf8');
const fixture = readFileSync(resolve(here, '../../../../tests/fixtures/site/tab-order/index.html'), 'utf8');
const URL = 'http://fixture.test/tab-order/index.html';

interface Entry {
  index: number;
  selector: string;
  path: string;
  tabindex: number;
  warnings: string[];
}

let browser: Browser;
beforeAll(async () => {
  browser = await chromium.launch({ headless: true });
});
afterAll(async () => {
  await browser?.close();
});

async function open(width: number): Promise<Page> {
  const context = await browser.newContext({ viewport: { width, height: 800 } });
  const page = await context.newPage();
  await page.route(URL, (route) => route.fulfill({ contentType: 'text/html', body: fixture }));
  await page.goto(URL);
  await page.waitForFunction(() => document.querySelector('iframe')?.contentDocument?.querySelector('button') !== null);
  return page;
}

/** What `overlay_tab_order` measures in the view (the expression src/tab_order.rs builds). */
async function measured(page: Page): Promise<Entry[]> {
  const cdp = await page.context().newCDPSession(page);
  const expression = `(() => {\n${measureBundle}\nreturn __rbOverlaysMeasure.tabOrder({ includeIframes: true });\n})()`;
  const result = await cdp.send('Runtime.evaluate', { expression, returnByValue: true });
  expect(result.exceptionDetails).toBeUndefined();
  return (result.result.value as { items: Entry[] }).items;
}

/** The data-t of the element each composed selector (`a >>> b` through shadow roots and
 *  iframes) finds. */
function names(page: Page, entries: Entry[]): Promise<(string | null)[]> {
  return page.evaluate(
    (selectors) =>
      selectors.map((selector) => {
        let scope: Document | ShadowRoot = document;
        let found: Element | null = null;
        for (const part of selector.split(' >>> ')) {
          found = scope.querySelector(part);
          if (!found) return null;
          scope = found.shadowRoot ?? (found as HTMLIFrameElement).contentDocument ?? scope;
        }
        return found?.getAttribute('data-t') ?? null;
      }),
    entries.map((entry) => entry.selector),
  );
}

/** The data-t of the element each of `count` presses of Tab focuses. */
async function tabbed(page: Page, count: number): Promise<(string | null)[]> {
  const found: (string | null)[] = [];
  for (let press = 0; press < count; press += 1) {
    await page.keyboard.press('Tab');
    found.push(
      await page.evaluate(() => {
        let active: Element | null = document.activeElement;
        for (;;) {
          const inner: Element | null | undefined =
            active?.shadowRoot?.activeElement ?? (active?.localName === 'iframe' ? (active as HTMLIFrameElement).contentDocument?.activeElement : undefined);
          if (!inner || inner.localName === 'body') break;
          active = inner;
        }
        return active?.getAttribute('data-t') ?? null;
      }),
    );
  }
  return found;
}

const warned = (entries: Entry[], found: (string | null)[], warning: string): (string | null)[] =>
  entries.flatMap((entry, place) => (entry.warnings.some((each) => each.startsWith(warning)) ? [found[place] ?? null] : []));

describe('the tab order, as the component measures it', () => {
  for (const width of [375, 1024]) {
    it(`numbers the stops as Tab really takes them at ${width} px`, async () => {
      const page = await open(width);
      const entries = await measured(page);
      const found = await names(page, entries);
      expect(entries.map((entry) => entry.index)).toEqual(entries.map((_, place) => place + 1));
      expect(await tabbed(page, entries.length)).toEqual(found);
      await page.context().close();
    });
  }

  it('puts tabindex 3 first with its warning, and the shadow content at its host', async () => {
    const page = await open(1024);
    const entries = await measured(page);
    const found = await names(page, entries);
    expect(found).toEqual(['promo', 'nav-home', 'nav-shop', 'first', 'second', 'inner', 'slotted', 'ghost', 'name', 'framed']);
    expect(entries[0]?.warnings).toContain('positive-tabindex');
    expect(entries[0]?.tabindex).toBe(3);
    expect(entries[5]?.selector).toContain(' >>> ');
    expect(warned(entries, found, 'reverse-jump')).toContain('second');
    await page.context().close();
  });

  it('warns about the closed menu in the mobile view only', async () => {
    const mobile = await open(375);
    const mobileEntries = await measured(mobile);
    expect(warned(mobileEntries, await names(mobile, mobileEntries), 'invisible-')).toEqual(['nav-home', 'nav-shop', 'ghost']);
    await mobile.context().close();
    const desktop = await open(1024);
    const desktopEntries = await measured(desktop);
    expect(warned(desktopEntries, await names(desktop, desktopEntries), 'invisible-')).toEqual(['ghost']);
    await desktop.context().close();
  });
});

describe('the page script', () => {
  it('draws the stops and leaves the DOM as it was when switched off', async () => {
    const page = await open(1024);
    await page.evaluate(() => {
      const listeners: ((message: unknown) => void)[] = [];
      const emitted: unknown[] = [];
      Object.assign(window, {
        __emitted: emitted,
        __push: (all: unknown) => {
          for (const listener of listeners) listener({ type: 'state', state: { version: 1, all, views: {} }, settings: {} });
        },
        rbPlugin: Object.freeze({
          id: 'overlays',
          view: 1,
          maxMessageBytes: 65536,
          send: (message: { type?: string }) => Promise.resolve(message.type === 'hello' ? { state: { version: 1, all: {}, views: {} }, settings: {} } : {}),
          emit: (message: unknown) => emitted.push(message),
          on: (listener: (message: unknown) => void) => {
            listeners.push(listener);
            return () => undefined;
          },
          onStop: () => () => undefined,
        }),
      });
    });
    const cdp = await page.context().newCDPSession(page);
    expect((await cdp.send('Runtime.evaluate', { expression: pageScript })).exceptionDetails).toBeUndefined();
    await page.waitForTimeout(100);
    const before = await page.evaluate(() => document.documentElement.outerHTML);
    const push = async (enabled: boolean): Promise<void> => {
      await page.evaluate((enabled) => (window as unknown as { __push(all: unknown): void }).__push({ 'tab-order': { enabled } }), enabled);
      await page.waitForTimeout(100);
    };
    await push(true);
    const emitted = (await page.evaluate(() => (window as unknown as { __emitted: unknown[] }).__emitted)) as { type: string; overlay?: string; count?: number }[];
    expect(emitted).toContainEqual({ type: 'hits', overlay: 'tab-order', count: 10 });
    expect(await page.evaluate(() => document.querySelector('rbrowser-overlays') !== null)).toBe(true);
    await push(false);
    expect(await page.evaluate(() => document.documentElement.outerHTML)).toBe(before);
    await page.context().close();
  });
});
