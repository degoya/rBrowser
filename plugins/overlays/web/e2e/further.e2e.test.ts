// The further overlays (RB-070-07) against e2e/fixtures/overlays.html in a real headless
// Chromium (`npm run build && npm run test:e2e`): the scans the component evaluates, the page
// script switching each overlay on and off again (the DOM as before), the forced :hover over CDP
// in two views at once, and the plugin's sheets under a strict CSP. The page script runs in the
// page's main world here, with a fake `rbPlugin` whose posts the test sends.

import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { type Browser, chromium, type Page } from 'playwright-core';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const pageScript = readFileSync(resolve(here, '../../page/overlays.js'), 'utf8');
const measureBundle = readFileSync(resolve(here, '../../assets/measure.js'), 'utf8');
const fixture = readFileSync(resolve(here, 'fixtures/overlays.html'), 'utf8');
const URL = 'http://fixture.test/overlays.html';

interface Hit {
  selector: string;
  label: string;
}

let browser: Browser;
beforeAll(async () => {
  // Headless Chromium hides scrollbars by default; the scrollbar overlay needs them shown.
  browser = await chromium.launch({ headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
});
afterAll(async () => {
  await browser?.close();
});

async function open(width: number, csp?: string): Promise<Page> {
  const context = await browser.newContext({ viewport: { width, height: 800 } });
  const page = await context.newPage();
  await page.route(URL, (route) =>
    route.fulfill({ contentType: 'text/html', body: fixture, headers: csp ? { 'content-security-policy': csp } : {} }),
  );
  await page.goto(URL);
  return page;
}

async function evaluate(page: Page, expression: string): Promise<unknown> {
  const cdp = await page.context().newCDPSession(page);
  const result = await cdp.send('Runtime.evaluate', { expression, returnByValue: true });
  expect(result.exceptionDetails).toBeUndefined();
  return result.result.value;
}

/** The expression src/scan.rs `script` builds. */
const scanScript = (overlay: string, options: Record<string, unknown> = {}): string =>
  `(() => {\n${measureBundle}\nreturn __rbOverlaysMeasure.scan(${JSON.stringify(overlay)}, ${JSON.stringify(options)});\n})()`;

async function scan(page: Page, overlay: string, options: Record<string, unknown> = {}): Promise<{ count: number; items: Hit[] }> {
  return (await evaluate(page, scanScript(overlay, options))) as { count: number; items: Hit[] };
}

/** Runs the page script with every overlay off; `push(page, all)` posts a new state. */
async function inject(page: Page): Promise<void> {
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
        send: (message: { type?: string }) =>
          Promise.resolve(message.type === 'hello' ? { state: { version: 1, all: {}, views: {} }, settings: {} } : {}),
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
  const result = await cdp.send('Runtime.evaluate', { expression: pageScript });
  expect(result.exceptionDetails).toBeUndefined();
  await page.waitForTimeout(100);
}

async function push(page: Page, all: Record<string, unknown>): Promise<void> {
  await page.evaluate((all) => (window as unknown as { __push(all: unknown): void }).__push(all), all);
  await page.waitForTimeout(100);
}

/** The DOM and the style state a page script could leave behind. */
const snapshot = (page: Page): Promise<string> =>
  page.evaluate(() =>
    JSON.stringify({
      html: document.documentElement.outerHTML,
      adopted: document.adoptedStyleSheets.length,
      disabled: Array.from(document.styleSheets).map((sheet) => sheet.disabled),
    }),
  );

describe('the checking overlays, as the component scans', () => {
  it('find the stacking contexts, the needless scrollbar and the small text', async () => {
    const page = await open(1024);
    const stacking = await scan(page, 'stacking');
    expect(stacking.items.map((hit) => [hit.selector, hit.label])).toEqual([
      ['section.layered', 'z:5 position + z-index'],
      ['div.faded', 'z:auto opacity'],
    ]);
    const scrollbars = await scan(page, 'scrollbars', { tolerancePx: 4 });
    expect(scrollbars.items.map((hit) => hit.selector)).toEqual(['div.scrolly']);
    const small = await scan(page, 'small-text', { px: 12 });
    expect(small.items.map((hit) => [hit.selector, hit.label])).toEqual([['p.fine', '10px']]);
    await page.context().close();
  });

  it('mark two 20 x 20 buttons 2 px apart, not two 24 px apart', async () => {
    const page = await open(1024);
    const targets = await scan(page, 'target-size', { sizePx: 24 });
    expect(targets.items.map((hit) => [hit.selector, hit.label])).toEqual([
      ['div.row.tight > button:nth-of-type(1)', '20x20'],
      ['div.row.tight > button:nth-of-type(2)', '20x20'],
    ]);
    await page.context().close();
  });

  it('report the card with a fixed height as clipped under WCAG text spacing, and take the spacing off again', async () => {
    const page = await open(1024);
    const before = await snapshot(page);
    const clipped = await scan(page, 'text-spacing');
    const card = clipped.items.find((hit) => hit.selector === 'div.card');
    expect(card?.label).toMatch(/^clipped \+\d+px$/);
    expect(await snapshot(page)).toBe(before);
    await page.context().close();
  });
});

describe('the page script', () => {
  it('switches each overlay on and off and leaves the DOM as it was', async () => {
    const page = await open(1024);
    await inject(page);
    const before = await snapshot(page);
    const cases: [string, Record<string, unknown>, () => unknown, unknown][] = [
      ['text-spacing', {}, () => getComputedStyle(document.querySelector('.card') as Element).lineHeight, '21px'],
      ['images-off', {}, () => getComputedStyle(document.querySelector('.hero') as Element).visibility, 'hidden'],
      ['css-off', {}, () => getComputedStyle(document.querySelector('.layered') as Element).position, 'static'],
      ['placeholdifier', {}, () => getComputedStyle(document.querySelector('.fine') as Element).color, 'rgba(0, 0, 0, 0)'],
      ['content-chaos', { factor: 2 }, () => (document.querySelector('.fine')?.textContent ?? '').length > 60, true],
      ['focus-tracking', {}, () => document.querySelector('rbrowser-overlays') !== null, true],
    ];
    for (const [overlay, options, probe, expected] of cases) {
      await push(page, { [overlay]: { enabled: true, options } });
      expect(await page.evaluate(probe), overlay).toBe(expected);
      await push(page, { [overlay]: { enabled: false } });
      expect(await snapshot(page), overlay).toBe(before);
    }
    await page.context().close();
  });

  it('tells the panel the hits and adds no overflow of its own', async () => {
    const page = await open(1024);
    await inject(page);
    const width = await page.evaluate(() => document.documentElement.scrollWidth);
    await push(page, {
      stacking: { enabled: true },
      scrollbars: { enabled: true },
      'target-size': { enabled: true },
      'small-text': { enabled: true },
      'force-state': { enabled: true, options: { selector: '.menu a', states: ['hover'] } },
    });
    const emitted = (await page.evaluate(() => (window as unknown as { __emitted: unknown[] }).__emitted)) as { type: string; overlay?: string; count?: number }[];
    const hits = Object.fromEntries(emitted.filter((message) => message.type === 'hits').map((message) => [message.overlay, message.count]));
    expect(hits).toEqual({ stacking: 2, scrollbars: 1, 'target-size': 2, 'small-text': 1, 'force-state': 1 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width);
    await page.context().close();
  });

  it('applies its sheets under a strict CSP (style-src none)', async () => {
    const page = await open(1024, "default-src 'none'; style-src 'none'; script-src 'none'");
    await inject(page);
    await push(page, { 'text-spacing': { enabled: true } });
    expect(await page.evaluate(() => getComputedStyle(document.body).letterSpacing)).not.toBe('normal');
    await page.context().close();
  });
});

describe('force-state over CDP, as src/effects.rs sends it', () => {
  it('forces :hover on the menu item in two views at once and releases it', async () => {
    const views = [await open(375), await open(1024)];
    const color = (page: Page) => page.evaluate(() => getComputedStyle(document.querySelector('.menu a') as Element).color);
    const sessions = await Promise.all(views.map((page) => page.context().newCDPSession(page)));
    const nodes = await Promise.all(
      sessions.map(async (cdp) => {
        await cdp.send('DOM.enable');
        await cdp.send('CSS.enable');
        const { root } = await cdp.send('DOM.getDocument', { depth: 0 });
        const { nodeId } = await cdp.send('DOM.querySelector', { nodeId: root.nodeId, selector: '.menu a' });
        await cdp.send('CSS.forcePseudoState', { nodeId, forcedPseudoClasses: ['hover'] });
        return nodeId;
      }),
    );
    expect(await Promise.all(views.map(color))).toEqual(['rgb(255, 0, 0)', 'rgb(255, 0, 0)']);
    await Promise.all(sessions.map((cdp, index) => cdp.send('CSS.forcePseudoState', { nodeId: nodes[index] ?? 0, forcedPseudoClasses: [] })));
    expect(await Promise.all(views.map(color))).toEqual(['rgb(0, 0, 255)', 'rgb(0, 0, 255)']);
    for (const page of views) await page.context().close();
  });
});

describe('js-off over CDP, as src/effects.rs sends it', () => {
  it('disables the page scripts, reloads from the plugin world, and enables them again', async () => {
    const page = await open(1024);
    await page.evaluate(() => document.body.setAttribute('data-ran', 'before'));
    const cdp = await page.context().newCDPSession(page);
    const reload = async (): Promise<void> => {
      const { frameTree } = await cdp.send('Page.getFrameTree');
      const { executionContextId } = await cdp.send('Page.createIsolatedWorld', { frameId: frameTree.frame.id, worldName: 'rbplugin-overlays' });
      const loaded = page.waitForEvent('load');
      await cdp.send('Runtime.evaluate', { expression: '(() => { location.reload(); return true; })()', contextId: executionContextId });
      await loaded;
    };
    const ran = () => page.evaluate(() => typeof (window as unknown as { __ranInline?: boolean }).__ranInline);
    await page.route(URL, (route) =>
      route.fulfill({ contentType: 'text/html', body: fixture.replace('</body>', '<script>window.__ranInline = true;</script></body>') }),
    );
    await cdp.send('Emulation.setScriptExecutionDisabled', { value: true });
    await reload();
    expect(await ran()).toBe('undefined');
    await cdp.send('Emulation.setScriptExecutionDisabled', { value: false });
    await reload();
    expect(await ran()).toBe('boolean');
    await page.context().close();
  });
});
