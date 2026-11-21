// The built page script and analysis against tests/fixtures/containers.html in a real headless
// Chromium (`npm run build && npm run test:e2e`), RB-094-01's acceptance with the browser's own
// layout: 3 containers with their sizes, exactly 4 conditions with their bounds, a container
// dragged on its handle to 400 px no longer matches (min-width: 500px) - the browser agrees, its
// rule no longer applies - and a reset brings back its inline style. The page script runs in the
// page's main world here, with a fake `rbPlugin`; in rBrowser it runs in the plugin's isolated world.

import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { type Browser, chromium, type Page } from 'playwright-core';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const pageScript = readFileSync(resolve(here, '../../page/containers.js'), 'utf8');
const analysis = readFileSync(resolve(here, '../../assets/analyze.js'), 'utf8');
const fixture = readFileSync(resolve(here, '../../tests/fixtures/containers.html'), 'utf8');
const URL = 'http://fixture.test/containers.html';

/** The expression the component evaluates (src/script.rs). */
const call = (expression: string): string => `(() => {\n${analysis}\nreturn __rbContainers.${expression};\n})()`;

interface Report {
  containers: { selector: string; names: string[]; inline: number; block: number; matching: number[]; resized: boolean }[];
  conditions: { name: string; query: string; bounds: { axis: string; op: string; px: number }[] }[];
}

let browser: Browser;
beforeAll(async () => {
  browser = await chromium.launch({ headless: true });
});
afterAll(async () => {
  await browser?.close();
});

async function open(): Promise<Page> {
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const page = await context.newPage();
  await page.route(URL, (route) => route.fulfill({ contentType: 'text/html', body: fixture }));
  await page.goto(URL);
  return page;
}

const analyze = (page: Page): Promise<Report> => page.evaluate(call('analyze()')) as Promise<Report>;
const titleSize = (page: Page): Promise<string> => page.evaluate(() => getComputedStyle(document.querySelector('.card .title') as Element).fontSize);

describe('the analysis in Chromium', () => {
  it('finds the 3 containers with their sizes and exactly the 4 conditions with their bounds', async () => {
    const page = await open();
    const report = await analyze(page);
    expect(report.containers.map((c) => [c.selector, c.names, c.inline])).toEqual([
      ['main.shell', ['shell'], 1000],
      ['article#hero', ['card'], 640],
      ['aside.side', [], 300],
    ]);
    expect(report.containers[1]?.block).toBe(300);
    expect(report.containers[2]?.block).toBe(250);
    expect(report.conditions.map((c) => [c.name, c.query, c.bounds.map((b) => `${b.axis} ${b.op} ${b.px}`)])).toEqual([
      ['card', '(min-width: 500px)', ['inline >= 500']],
      ['card', '(max-width: 300px)', ['inline <= 300']],
      ['shell', '(400px <= width < 900px)', ['inline >= 400', 'inline < 900']],
      ['', '(min-height: 200px)', ['block >= 200']],
    ]);
    expect(report.containers.map((c) => c.matching)).toEqual([[], [0], [3]]);
    expect(await titleSize(page)).toBe('32px');
    await page.context().close();
  });

  it('resizes a container to 400 px and resets it, as containers_resize does', async () => {
    const page = await open();
    const before = await page.evaluate(() => (document.getElementById('hero') as HTMLElement).style.cssText);
    expect(await page.evaluate(call('resize("#hero", 400)'))).toBe(true);
    const resized = await analyze(page);
    expect(resized.containers[1]).toMatchObject({ inline: 400, matching: [], resized: true });
    expect(await titleSize(page)).toBe('16px');
    expect(await page.evaluate(call('reset(null)'))).toBe(1);
    expect(await page.evaluate(() => (document.getElementById('hero') as HTMLElement).style.cssText)).toBe(before);
    expect((await analyze(page)).containers[1]?.matching).toEqual([0]);
    await page.context().close();
  });
});

describe('a border-box container in Chromium', () => {
  it('gets the content width asked for, its padding and border on top', async () => {
    const page = await open();
    await page.evaluate(() => {
      const hero = document.getElementById('hero') as HTMLElement;
      for (const [name, value] of [['box-sizing', 'border-box'], ['padding', '0 16px'], ['border', '1px solid black']]) hero.style.setProperty(name as string, value as string);
    });
    expect((await analyze(page)).containers[1]?.inline).toBe(606);
    expect(await page.evaluate(call('resize("#hero", 400)'))).toBe(true);
    expect((await analyze(page)).containers[1]).toMatchObject({ inline: 400, matching: [] });
    expect(await page.evaluate(() => (document.getElementById('hero') as HTMLElement).getBoundingClientRect().width)).toBe(434);
    await page.context().close();
  });
});

describe('the page script in Chromium', () => {
  it('frames the containers and drags one to 400 px on its handle; a double click resets it', async () => {
    const page = await open();
    await page.evaluate((source) => {
      const roots: ShadowRoot[] = [];
      const attach = HTMLElement.prototype.attachShadow;
      HTMLElement.prototype.attachShadow = function (this: HTMLElement, init: ShadowRootInit) {
        const root = attach.call(this, init);
        roots.push(root);
        return root;
      };
      const emitted: unknown[] = [];
      const plugin = {
        id: 'container-queries',
        view: 1,
        maxMessageBytes: 65536,
        layers: {
          add: (host: Element) => {
            document.documentElement.append(host);
            return true;
          },
          remove: (host: Element) => host.remove(),
        },
        send: () => Promise.resolve({ on: true, settings: { labels: true, color: '#0d9488' } }),
        emit: (message: unknown) => emitted.push(message),
        on: () => () => undefined,
        onStop: () => () => undefined,
      };
      Object.assign(window, { __roots: roots, __emitted: emitted });
      new Function('rbPlugin', `'use strict';\n${source}`)(plugin);
    }, pageScript);
    await page.waitForFunction(() => ((window as unknown as { __roots: ShadowRoot[] }).__roots[0]?.firstElementChild?.children.length ?? 0) === 3);
    const labels = await page.evaluate(() =>
      Array.from((window as unknown as { __roots: ShadowRoot[] }).__roots[0]?.firstElementChild?.children ?? [], (box) => box.children[0]?.textContent),
    );
    expect(labels[1]).toBe('card · 640 × 300 · ✓ (min-width: 500px)');
    const handle = await page.evaluate(() => {
      const box = (window as unknown as { __roots: ShadowRoot[] }).__roots[0]?.firstElementChild?.children[1];
      const rect = (box?.children[1] as HTMLElement).getBoundingClientRect();
      return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
    });
    await page.mouse.move(handle.x, handle.y);
    await page.mouse.down();
    await page.mouse.move(handle.x - 120, handle.y, { steps: 4 });
    await page.mouse.move(handle.x - 240, handle.y, { steps: 4 });
    await page.mouse.up();
    expect(await page.evaluate(() => (document.getElementById('hero') as HTMLElement).getBoundingClientRect().width)).toBe(400);
    expect(await titleSize(page)).toBe('16px');
    expect(await page.evaluate(() => (window as unknown as { __emitted: unknown[] }).__emitted)).toEqual([{ type: 'resized', path: 'html > body > main > article', width: 400 }]);
    await page.mouse.dblclick(handle.x - 240, handle.y);
    expect(await page.evaluate(() => (document.getElementById('hero') as HTMLElement).hasAttribute('data-rbrowser-container-width'))).toBe(false);
    expect(await titleSize(page)).toBe('32px');
    await page.context().close();
  });
});
