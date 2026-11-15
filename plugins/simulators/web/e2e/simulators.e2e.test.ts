// The page script in a real headless Chromium (RB-070-08's acceptance without the app):
// `npm run build && npm run test:e2e`. The page script runs in the page's main world here, with a
// stand-in `rbPlugin` whose posts the test sends; the drawings are the component's
// (`tests/fixtures/specs.json`, held to src/catalog.rs by `cargo test`).
//
// - Every one of the fifteen simulations against the colour chart or the flat page, pixel by
//   pixel against reference.ts's arithmetic (6 of 255 per channel, well inside
//   `visual.threshold` 0.1).
// - The four colour types Chromium renders itself, beside the page script's drawing of them.
// - Two pages at once with different simulations.
// - A `position: fixed` header under the filter, and the control: the same filter on <body>
//   moves it.
// - Clicks and hit tests through the layers.

import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { type Browser, chromium, type Page } from 'playwright-core';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { distance, filtered, hex, over, type Rgb, type Spec } from './reference';

const here = dirname(fileURLToPath(import.meta.url));
const fixtures = resolve(here, '../../tests/fixtures');
const pageScript = readFileSync(resolve(here, '../../page/simulators.js'), 'utf8');
const specs = JSON.parse(readFileSync(resolve(fixtures, 'specs.json'), 'utf8')) as Record<string, Spec>;
const CHART = readFileSync(resolve(fixtures, 'color-chart.html'), 'utf8');
const FLAT = readFileSync(resolve(fixtures, 'flat.html'), 'utf8');
const TOLERANCE = 6;

/** The chart's swatches: colour and centre (body padding 40/80, 100 px swatches, 20 px gaps). */
const SWATCHES: Array<{ color: Rgb; at: [number, number] }> = [
  '#e53935', '#43a047', '#1e88e5', '#fdd835', '#8e24aa', '#fb8c00',
  '#00acc1', '#6d4c41', '#ff00ff', '#808080', '#000000', '#c0ca33',
].map((color, index) => ({ color: hex(color), at: [90 + (index % 4) * 120, 130 + Math.floor(index / 4) * 120] }));
const FLAT_COLOR = hex('#3366cc');
const WIDTH = 800;
const HEIGHT = 600;

let browser: Browser;
let reader: Page;
beforeAll(async () => {
  browser = await chromium.launch({ headless: true });
  reader = await browser.newPage();
});
afterAll(async () => {
  await browser?.close();
});

async function open(html: string): Promise<Page> {
  const page = await browser.newPage({ viewport: { width: WIDTH, height: HEIGHT }, deviceScaleFactor: 1 });
  await page.route('http://fixture.test/', (route) => route.fulfill({ contentType: 'text/html', body: html }));
  await page.goto('http://fixture.test/');
  await page.evaluate(() => {
    const listeners: Array<(message: unknown) => void> = [];
    Object.assign(window, {
      __post: (message: unknown) => listeners.forEach((listener) => listener(message)),
      rbPlugin: Object.freeze({
        id: 'simulators',
        view: 1,
        maxMessageBytes: 65536,
        send: () => Promise.resolve({ spec: null }),
        emit: () => undefined,
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
  return page;
}

async function frames(page: Page): Promise<void> {
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(() => done(null)))));
}

async function show(page: Page, spec: Spec | null): Promise<void> {
  await page.evaluate((spec) => (window as unknown as { __post(message: unknown): void }).__post({ type: 'apply', spec }), spec);
  await frames(page);
}

async function native(page: Page, type: string): Promise<void> {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Emulation.setEmulatedVisionDeficiency', { type: type as 'none' });
  await frames(page);
}

/** The colours at `points` of what `page` shows now. */
async function pixels(page: Page, points: Array<[number, number]>): Promise<Rgb[]> {
  const png = (await page.screenshot({ type: 'png' })).toString('base64');
  return reader.evaluate(
    async ({ png, points }) => {
      const image = new Image();
      image.src = `data:image/png;base64,${png}`;
      await image.decode();
      const canvas = new OffscreenCanvas(image.width, image.height);
      const context = canvas.getContext('2d');
      if (!context) throw new Error('no 2d context');
      context.drawImage(image, 0, 0);
      return points.map(([x, y]) => Array.from(context.getImageData(x, y, 1, 1).data.slice(0, 3)) as [number, number, number]);
    },
    { png, points },
  );
}

const spec = (id: string): Spec => {
  const found = specs[id];
  if (!found) throw new Error(`no spec for ${id}`);
  return found;
};

describe('the colour chart, type by type', () => {
  const chartTypes = Object.keys(specs).filter((id) => !spec(id).overlay);

  it('the fixture holds all fifteen simulations', () => {
    expect(Object.keys(specs)).toHaveLength(15);
  });

  it.each(chartTypes)('%s matches its reference', async (id) => {
    const page = await open(CHART);
    await show(page, spec(id));
    const seen = await pixels(page, SWATCHES.map((swatch) => swatch.at));
    SWATCHES.forEach((swatch, index) => {
      const expected = filtered(swatch.color, spec(id).filter);
      expect(distance(seen[index]!, expected), `${id} swatch ${index}: ${seen[index]} vs ${expected}`).toBeLessThanOrEqual(TOLERANCE);
    });
    await page.close();
  });
});

describe('the layers on a flat page', () => {
  const corner: [number, number] = [8, 8];
  const centre: [number, number] = [WIDTH / 2, HEIGHT / 2];

  it('glaucoma keeps the centre and darkens the edges', async () => {
    const page = await open(FLAT);
    await show(page, spec('glaucoma'));
    const [atCorner, atCentre] = await pixels(page, [corner, centre]);
    const opacity = spec('glaucoma').overlay?.opacity as number;
    expect(distance(atCentre!, FLAT_COLOR)).toBeLessThanOrEqual(TOLERANCE);
    expect(distance(atCorner!, over(FLAT_COLOR, [0, 0, 0], opacity))).toBeLessThanOrEqual(TOLERANCE);
    await page.close();
  });

  it('macular degeneration darkens the centre and keeps the edges', async () => {
    const page = await open(FLAT);
    await show(page, spec('macular-degeneration'));
    const [atCorner, atCentre] = await pixels(page, [corner, centre]);
    const opacity = spec('macular-degeneration').overlay?.opacity as number;
    expect(distance(atCorner!, FLAT_COLOR)).toBeLessThanOrEqual(TOLERANCE);
    expect(distance(atCentre!, over(FLAT_COLOR, [40, 40, 40], opacity))).toBeLessThanOrEqual(TOLERANCE);
    await page.close();
  });

  it('cataract clouds, yellows and adds glare, the page background included', async () => {
    const page = await open(FLAT);
    await show(page, spec('cataract'));
    const [atGlare] = await pixels(page, [[WIDTH / 2, Math.round(HEIGHT * 0.45)]]);
    const opacity = (spec('cataract').overlay?.opacity as number) ?? 0;
    const expected = filtered(over(FLAT_COLOR, [255, 255, 245], opacity), spec('cataract').filter);
    expect(distance(atGlare!, expected), `${atGlare} vs ${expected}`).toBeLessThanOrEqual(TOLERANCE);
    await page.close();
  });

  it('a dimmed screen dims the page background too', async () => {
    const page = await open(FLAT);
    await show(page, spec('dimmed'));
    const [atCorner] = await pixels(page, [corner]);
    expect(distance(atCorner!, filtered(FLAT_COLOR, spec('dimmed').filter))).toBeLessThanOrEqual(TOLERANCE);
    await page.close();
  });
});

describe("Chromium's own emulation", () => {
  it.each(['protanopia', 'deuteranopia', 'tritanopia', 'achromatopsia'])(
    '%s looks the same native and drawn by the page script',
    async (id) => {
      const page = await open(CHART);
      await native(page, id);
      const nativePixels = await pixels(page, SWATCHES.map((swatch) => swatch.at));
      await native(page, 'none');
      await show(page, spec(id));
      const drawn = await pixels(page, SWATCHES.map((swatch) => swatch.at));
      nativePixels.forEach((pixel, index) => {
        expect(distance(pixel, drawn[index]!), `${id} swatch ${index}: ${pixel} vs ${drawn[index]}`).toBeLessThanOrEqual(TOLERANCE);
      });
      await page.close();
    },
  );

  it('two views show two simulations at once', async () => {
    const [mobile, desktop] = await Promise.all([open(CHART), open(CHART)]);
    await native(mobile!, 'deuteranopia');
    await show(desktop!, spec('tritanomaly'));
    const red = SWATCHES[0]!;
    const [left] = await pixels(mobile!, [red.at]);
    const [right] = await pixels(desktop!, [red.at]);
    expect(distance(left!, filtered(red.color, spec('deuteranopia').filter))).toBeLessThanOrEqual(TOLERANCE);
    expect(distance(right!, filtered(red.color, spec('tritanomaly').filter))).toBeLessThanOrEqual(TOLERANCE);
    expect(distance(left!, right!)).toBeGreaterThan(20);
    await Promise.all([mobile!.close(), desktop!.close()]);
  });
});

describe('the page under the simulation', () => {
  const headerTop = (page: Page) =>
    page.evaluate(() => (document.getElementById('header') as HTMLElement).getBoundingClientRect().top);

  it('a position: fixed header stays put under the filter', async () => {
    const page = await open(CHART);
    await show(page, spec('tritanomaly'));
    expect(await page.evaluate(() => getComputedStyle(document.documentElement).filter)).toContain('url(');
    await page.mouse.wheel(0, 500);
    await page.waitForFunction(() => window.scrollY >= 500);
    expect(await headerTop(page)).toBe(0);
    // The header still paints at the top of the view, drawn through the filter.
    const [atHeader] = await pixels(page, [[400, 20]]);
    expect(distance(atHeader!, filtered(hex('#20304a'), spec('tritanomaly').filter))).toBeLessThanOrEqual(TOLERANCE);
    await page.close();
  });

  it('the control: the same filter on <body> would move it', async () => {
    const page = await open(CHART);
    await page.evaluate(() => document.body.style.setProperty('filter', 'contrast(0.5)'));
    await page.mouse.wheel(0, 500);
    await page.waitForFunction(() => window.scrollY >= 500);
    expect(await headerTop(page)).toBeLessThan(0);
    await page.close();
  });

  it('clicks and hit tests go through the layers', async () => {
    for (const id of ['glaucoma', 'macular-degeneration', 'cataract']) {
      const page = await open(CHART);
      await show(page, spec(id));
      const box = await page.locator('#press').boundingBox();
      if (!box) throw new Error('no button');
      const [x, y] = [box.x + box.width / 2, box.y + box.height / 2];
      expect(await page.evaluate(([x, y]) => document.elementFromPoint(x!, y!)?.id, [x, y])).toBe('press');
      await page.mouse.click(x, y);
      expect(await page.evaluate(() => (window as unknown as { pressed?: number }).pressed)).toBe(1);
      await page.close();
    }
  });

  it('from document-start, and again after a reload, the view asks for its simulation and draws it', async () => {
    const page = await browser.newPage({ viewport: { width: WIDTH, height: HEIGHT }, deviceScaleFactor: 1 });
    await page.route('http://fixture.test/', (route) => route.fulfill({ contentType: 'text/html', body: CHART }));
    const answer = { spec: spec('dimmed') };
    await page.addInitScript(
      ({ answer, script }) => {
        Object.assign(window, {
          __asked: 0,
          rbPlugin: Object.freeze({
            id: 'simulators',
            view: 1,
            maxMessageBytes: 65536,
            send: () => {
              (window as unknown as { __asked: number }).__asked += 1;
              return Promise.resolve(answer);
            },
            emit: () => undefined,
            on: () => () => undefined,
            onStop: () => () => undefined,
          }),
        });
        new Function(script)();
      },
      { answer, script: pageScript },
    );
    for (const load of [() => page.goto('http://fixture.test/'), () => page.reload()]) {
      await load();
      await frames(page);
      expect(await page.evaluate(() => (window as unknown as { __asked: number }).__asked)).toBe(1);
      const [atSwatch] = await pixels(page, [SWATCHES[0]!.at]);
      expect(distance(atSwatch!, filtered(SWATCHES[0]!.color, spec('dimmed').filter))).toBeLessThanOrEqual(TOLERANCE);
    }
    await page.close();
  });

  it('off leaves the page as it was', async () => {
    const page = await open(CHART);
    const before = await page.evaluate(() => document.documentElement.outerHTML);
    await show(page, spec('cataract'));
    await show(page, null);
    expect(await page.evaluate(() => document.documentElement.outerHTML)).toBe(before);
    await page.close();
  });
});
