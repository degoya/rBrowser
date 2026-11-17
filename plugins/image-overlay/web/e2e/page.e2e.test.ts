// The page script in a real headless Chromium (RB-070-09's pixel acceptance without the app):
// `tests/fixtures/page.html` with a design that is the page's own screenshot, the page script run
// as the runtime runs it with a stand-in bridge that serves the design in the component's chunks.
// Pixel samples are read from Chromium's screenshots, decoded in a blank page.
//
// Needs Playwright's headless shell: `npx playwright-core install chromium-headless-shell`.
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { chromium, type Browser, type Page } from 'playwright-core';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const source = readFileSync(resolve(here, '../../page/image-overlay.js'), 'utf8');
const fixture = pathToFileURL(resolve(here, '../../tests/fixtures/page.html')).href;
const designs = resolve(here, '../../../../tests/fixtures/project/.rbrowser/designs');
/** As `CHUNK_BYTES` in `src/lib.rs`. */
const CHUNK = 47 * 1024;

let browser: Browser;

interface Overlay {
  blend?: string;
  opacity?: number;
  anchor?: 'document' | 'viewport';
  scale?: number;
  split?: { on: boolean; position: number };
}

/** The design served as the component serves it: its meta and its chunks as base64. */
function served(png: Buffer, scale: number) {
  const width = png.readUInt32BE(16);
  const height = png.readUInt32BE(20);
  const chunks: string[] = [];
  for (let start = 0; start < png.length; start += CHUNK) chunks.push(png.subarray(start, start + CHUNK).toString('base64'));
  const image = { path: 'designs/design.png', mime: 'image/png', bytes: png.length, hash: 'h', width, height, scale, chunks: chunks.length };
  return { image, chunks };
}

async function inject(page: Page, png: Buffer, overlay: Overlay): Promise<void> {
  const { image, chunks } = served(png, overlay.scale ?? 1);
  const state = {
    enabled: true,
    opacity: overlay.opacity ?? 100,
    blend: overlay.blend ?? 'normal',
    invert: false,
    anchor: overlay.anchor ?? 'document',
    offset: { x: 0, y: 0 },
    split: overlay.split ?? { on: false, position: 50 },
    autoScaleDpr: true,
    designs: {},
  };
  await page.evaluate(
    ([script, init, parts]) => {
      const bridge = {
        id: 'image-overlay',
        view: 1,
        maxMessageBytes: 65536,
        send: (message: { op: string; index?: number }) =>
          Promise.resolve(
            message.op === 'init'
              ? init
              : message.op === 'chunk'
                ? { index: message.index, data: parts[message.index ?? 0] }
                : { state: init.state },
          ),
        emit: () => undefined,
        on: () => () => undefined,
        onStop: () => () => undefined,
      };
      new Function('rbPlugin', `'use strict';\n${script}`)(bridge);
    },
    [source, { state, overrides: {}, image, problem: null, chunkBytes: CHUNK }, chunks] as const,
  );
  await page.waitForFunction(() => document.querySelector('rbrowser-image-overlay') !== null);
  // Two frames for the canvas to be composited.
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
}

/** The largest channel value over a grid of samples of `png` (0 for a black picture). */
async function brightest(png: Buffer): Promise<{ max: number; width: number }> {
  const page = await browser.newPage();
  try {
    return await page.evaluate(async (base64) => {
      const bytes = Uint8Array.from(atob(base64), (char) => char.charCodeAt(0));
      const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      const context = canvas.getContext('2d');
      if (!context) throw new Error('no 2d context');
      context.drawImage(bitmap, 0, 0);
      const { data } = context.getImageData(0, 0, bitmap.width, bitmap.height);
      let max = 0;
      for (let y = 0; y < bitmap.height; y += 7) {
        for (let x = 0; x < bitmap.width; x += 7) {
          const at = (y * bitmap.width + x) * 4;
          max = Math.max(max, data[at] ?? 0, data[at + 1] ?? 0, data[at + 2] ?? 0);
        }
      }
      return { max, width: bitmap.width };
    }, png.toString('base64'));
  } finally {
    await page.close();
  }
}

async function open(width: number, dpr: number): Promise<Page> {
  const context = await browser.newContext({ viewport: { width, height: 700 }, deviceScaleFactor: dpr });
  const page = await context.newPage();
  await page.goto(fixture);
  return page;
}

beforeAll(async () => {
  browser = await chromium.launch();
});

afterAll(async () => {
  await browser?.close();
});

describe('the image overlay in Chromium', () => {
  it('turns the page black under difference with its own screenshot as the design', async () => {
    const page = await open(393, 1);
    const design = await page.screenshot();
    expect((await brightest(design)).max).toBeGreaterThan(200);
    await inject(page, design, { blend: 'difference' });
    expect((await brightest(await page.screenshot())).max).toBeLessThanOrEqual(2);
    await page.context().close();
  });

  it('lays an @3x export over the DPR 3 view pixel for pixel', async () => {
    const page = await open(393, 3);
    const design = await page.screenshot();
    expect((await brightest(design)).width).toBe(1179);
    await inject(page, design, { blend: 'difference', scale: 3 });
    const shot = await page.screenshot();
    expect((await brightest(shot)).max).toBeLessThanOrEqual(2);
    await page.context().close();
  });

  it('scrolls a design pinned to the document with the page, and not one fixed in the view', async () => {
    const page = await open(393, 1);
    const design = await page.screenshot({ fullPage: true });
    await inject(page, design, { blend: 'difference' });
    await page.mouse.wheel(0, 900);
    await page.waitForFunction(() => window.scrollY >= 900);
    await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
    expect((await brightest(await page.screenshot())).max).toBeLessThanOrEqual(2);
    await page.context().close();

    const fixed = await open(393, 1);
    const whole = await fixed.screenshot({ fullPage: true });
    await inject(fixed, whole, { blend: 'difference', anchor: 'viewport' });
    await fixed.mouse.wheel(0, 900);
    await fixed.waitForFunction(() => window.scrollY >= 900);
    expect((await brightest(await fixed.screenshot())).max).toBeGreaterThan(40);
    await fixed.context().close();
  });

  it('shows the design left of the split only', async () => {
    const page = await open(393, 1);
    const design = await page.screenshot();
    await inject(page, design, { blend: 'difference', split: { on: true, position: 30 } });
    const shot = await page.screenshot({ clip: { x: 0, y: 0, width: 100, height: 700 } });
    expect((await brightest(shot)).max).toBeLessThanOrEqual(2);
    const right = await page.screenshot({ clip: { x: 200, y: 0, width: 193, height: 700 } });
    expect((await brightest(right)).max).toBeGreaterThan(200);
    await page.context().close();
  });

  it('decodes the fixture project designs at their size', async () => {
    for (const [name, width] of [
      ['mobile.png', 393],
      ['desktop.png', 1440],
    ] as const) {
      const page = await open(width, 1);
      await inject(page, readFileSync(resolve(designs, name)), { opacity: 100 });
      const size = await page.evaluate(() => {
        const host = document.querySelector('rbrowser-image-overlay');
        const rect = host?.getBoundingClientRect();
        return { width: rect?.width ?? 0, connected: Boolean(host?.isConnected) };
      });
      expect(size.connected).toBe(true);
      // The header of the design (its accent colour) now covers the page's header.
      const top = await brightest(await page.screenshot({ clip: { x: 0, y: 0, width, height: 60 } }));
      expect(top.max).toBeGreaterThan(150);
      await page.context().close();
    }
  });
});
