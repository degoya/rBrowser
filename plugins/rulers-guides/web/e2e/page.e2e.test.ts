// The page script in a real headless Chromium at DPR 1 and 3 (RB-060-05's acceptance without the
// app): `tests/fixtures/grid-bootstrap.html` at a mobile and a desktop width, the page script run
// as the runtime runs it with a stand-in bridge that answers the component's drawing for that
// width. The column numbers are the component's for the fixture's setups
// (`src/columns_tests.rs`, `the_bootstrap_fixture_setups_give_the_e2e_numbers`).
//
// Needs Playwright's headless shell: `npx playwright-core install chromium-headless-shell`.
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { chromium, type Browser, type Page } from 'playwright-core';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const source = readFileSync(resolve(here, '../../page/rulers.js'), 'utf8');
const fixture = pathToFileURL(resolve(here, '../../tests/fixtures/grid-bootstrap.html')).href;

interface Box {
  left: number;
  top: number;
  width: number;
}

function columns(width: number): Array<{ x: number; width: number }> {
  if (width < 1200) {
    return [8, 101.75, 195.5, 289.25].map((x) => ({ x, width: 77.75 }));
  }
  return Array.from({ length: 12 }, (_, index) => ({ x: 132 + index * 100, width: 76 }));
}

function drawing(width: number) {
  return {
    breakpoint: width < 1200 ? 'mobile' : 'desktop',
    visible: { rulers: true, columns: true, baseline: true },
    guides: [
      { id: 1, axis: 'x', pos: width / 2 },
      { id: 2, axis: 'x', pos: 320 },
      { id: 3, axis: 'y', pos: 320 },
    ],
    columns: { left: width < 1200 ? 0 : 120, width: Math.min(width, 1200), columns: columns(width) },
    baseline: { lineHeight: 8, offset: 0 },
  };
}

/** What the page measures without the overlay: its scroll width and what a point hits. */
async function probe(page: Page) {
  return page.evaluate(() => ({
    scrollWidth: document.documentElement.scrollWidth,
    hits: [
      [300, 300],
      [320, 320],
      [100, 400],
    ].map(([x, y]) => document.elementFromPoint(x ?? 0, y ?? 0)?.id || document.elementFromPoint(x ?? 0, y ?? 0)?.tagName),
  }));
}

async function inject(page: Page, width: number): Promise<void> {
  await page.evaluate(
    ([script, layout]) => {
      // The test opens the shadow root the page script closes, to measure what it drew.
      const attach = HTMLElement.prototype.attachShadow;
      HTMLElement.prototype.attachShadow = function (init) {
        return attach.call(this, { ...init, mode: 'open' });
      };
      const bridge = {
        id: 'rulers-guides',
        view: 1,
        maxMessageBytes: 65536,
        send: (message: { op: string }) =>
          Promise.resolve(
            message.op === 'init'
              ? { state: {}, overrides: {}, prefs: { unit: 'px', snapPx: 8 }, layout }
              : { layout },
          ),
        emit: () => undefined,
        on: () => () => undefined,
        onStop: () => () => undefined,
      };
      new Function('rbPlugin', `'use strict';\n${script}`)(bridge);
      HTMLElement.prototype.attachShadow = attach;
    },
    [source, drawing(width)] as const,
  );
  await page.waitForFunction(() => {
    const host = document.querySelector('[data-rbrowser-plugin]');
    return host instanceof HTMLElement && host.style.display === 'block';
  });
}

/** Rects of `selector` inside the overlay's (test-opened) shadow root. */
async function overlay(page: Page, selector: string): Promise<Box[]> {
  return page.evaluate((query) => {
    const root = document.querySelector('[data-rbrowser-plugin]')?.shadowRoot;
    return [...(root?.querySelectorAll(query) ?? [])].map((node) => {
      const rect = node.getBoundingClientRect();
      return { left: rect.left, top: rect.top, width: rect.width };
    });
  }, selector);
}

async function pageRects(page: Page, selector: string): Promise<Box[]> {
  return page.evaluate(
    (query) =>
      [...document.querySelectorAll(query)].map((node) => {
        const rect = node.getBoundingClientRect();
        return { left: rect.left, top: rect.top, width: rect.width };
      }),
    selector,
  );
}

let browser: Browser;

beforeAll(async () => {
  browser = await chromium.launch({ headless: true });
});

afterAll(async () => {
  await browser?.close();
});

describe.each([1, 3])('the page script at DPR %i', (dpr) => {
  describe.each([375, 1440])('in a %i px wide view', (width) => {
    async function open(): Promise<Page> {
      const context = await browser.newContext({ viewport: { width, height: 800 }, deviceScaleFactor: dpr });
      const page = await context.newPage();
      await page.goto(fixture);
      return page;
    }

    it('draws the columns on the fixture grid, pixel for pixel', async () => {
      const page = await open();
      await inject(page, width);
      const drawn = await overlay(page, '.columns div');
      const count = width < 1200 ? 4 : 12;
      const cells = (await pageRects(page, '.cell')).slice(0, count);
      expect(drawn).toHaveLength(count);
      drawn.forEach((column, index) => {
        expect(column.left).toBeCloseTo(cells[index]?.left ?? NaN, 2);
        expect(column.width).toBeCloseTo(cells[index]?.width ?? NaN, 2);
      });
      await page.context().close();
    });

    it('reads 200 on the ruler where an element at left: 200px lies, scrolled too', async () => {
      const page = await open();
      await inject(page, width);
      const [mark] = await pageRects(page, '#mark');
      const [label] = await overlay(page, '.top .label[data-value="200"]');
      expect(Math.abs((label?.left ?? NaN) - (mark?.left ?? NaN))).toBeLessThanOrEqual(1);
      await page.evaluate(() => window.scrollTo(0, 150));
      await page.waitForFunction(() => window.scrollY === 150);
      await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
      const [scrolledMark] = await pageRects(page, '#mark');
      const [left] = await overlay(page, '.left .label[data-value="200"]');
      expect(Math.abs((left?.top ?? NaN) - (scrolledMark?.top ?? NaN))).toBeLessThanOrEqual(1);
      await page.context().close();
    });

    it('puts the 50 % guide in the middle and the 320 px guide at 320', async () => {
      const page = await open();
      await inject(page, width);
      const guides = await overlay(page, '.guides-x .guide');
      const middle = await page.evaluate(() => document.documentElement.clientWidth / 2);
      expect(guides[0]?.left).toBeCloseTo(middle, 2);
      expect(guides[1]?.left).toBeCloseTo(320, 2);
      await page.context().close();
    });

    it('changes neither the scroll width nor the hit tests outside the rulers', async () => {
      const page = await open();
      const before = await probe(page);
      await inject(page, width);
      expect(await probe(page)).toEqual(before);
      // On the rulers the overlay takes the pointer.
      const onRuler = await page.evaluate(() => document.elementFromPoint(200, 8)?.getAttribute('data-rbrowser-plugin'));
      expect(onRuler).toBe('rulers-guides');
      await page.context().close();
    });
  });
});
