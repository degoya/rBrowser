// The collector's answers for the acceptance pages tests/fixtures/site/fonts/ - index.html in a
// wide and a narrow view, broken.html (Inter cannot load) in a wide one - committed as
// plugins/fonts/tests/fixtures/{wide,narrow,broken}.json: the component's
// native tests (tests/fixture.rs) take them as the views' pages, so the page, the collector and the
// findings are tested end to end without a browser. `FONTS_UPDATE_FIXTURES=1 npm test` writes
// them; otherwise this fails when they are stale. Characters beyond ASCII are written as \u
// escapes (the repository's English-only lint reads plugins/**/*.json).

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import broken from '../../../../tests/fixtures/site/fonts/broken.html?raw';
import fixture from '../../../../tests/fixtures/site/fonts/index.html?raw';
import { collect, type Env } from './collect';
import type { Box } from './types';

const OUT = resolve(dirname(fileURLToPath(import.meta.url)), '../../tests/fixtures');
const BASE = 'http://localhost:8080/fonts/';

/** `document.fonts` as Chromium has it after the page loaded: Legacy Serif's file is missing,
 * Unused Display was never needed. */
const STATUSES = [
  ['Inter', '400', 'swap', 'loaded'],
  ['Inter', '600', 'auto', 'loaded'],
  ['Roboto', '400', 'swap', 'loaded'],
  ['Legacy Serif', '400', 'swap', 'error'],
  ['Unused Display', '400', 'swap', 'unloaded'],
  ['Inter ASCII', '400', 'swap', 'loaded'],
].map(([family, weight, display, status]) => ({
  family: family ?? '',
  weight: weight ?? '',
  style: 'normal',
  display: display ?? '',
  unicodeRange: 'U+0-10FFFF',
  status: status ?? '',
}));

/** The font files as Resource Timing lists them: Google answers with `Timing-Allow-Origin`. */
const RESOURCES = [
  ['http://localhost:8080/fonts/inter-400.woff2', 17156],
  ['http://localhost:8080/fonts/inter-600.woff2', 18096],
  ['https://fonts.gstatic.com/s/roboto/v30/KFOmCnqEu92Fr1Mu4mxK.woff2', 15744],
].map(([name, size]) => ({
  name: String(name),
  initiatorType: 'css',
  encodedBodySize: Number(size),
  transferSize: Number(size) + 300,
  decodedBodySize: Number(size),
}));

function ascii(text: string): string {
  return text.replace(/[\u0080-￿]/g, (char) => `\\u${char.charCodeAt(0).toString(16).padStart(4, '0')}`);
}

/** broken.html: its one face failed, nothing loaded. */
const BROKEN = [{ family: 'Inter', weight: '400', style: 'normal', display: 'swap', unicodeRange: 'U+0-10FFFF', status: 'error' }];

function fontsAt(page: string, html: string, width: number): string {
  const happy = (window as unknown as { happyDOM: { setURL(url: string): void; setViewport(v: { width: number; height: number }): void } }).happyDOM;
  happy.setURL(BASE + page);
  happy.setViewport({ width, height: 800 });
  document.head.innerHTML =
    `<style>${/<style>([\s\S]*?)<\/style>/.exec(html)?.[1] ?? ''}</style>` +
    (/<link rel="preload"[^>]*>/.exec(html)?.[0] ?? '');
  document.body.innerHTML = /<body>([\s\S]*)<\/body>/.exec(html)?.[1] ?? '';
  // The page's real geometry is the browser's; here the header is on the first screen, the rest
  // below it.
  const env: Env = {
    rect: (element): Box => (element.closest('header') || page === 'broken.html' ? [16, 20, width - 32, 40] : [16, 900, width - 32, 24]),
    viewport: () => ({ width, height: 800 }),
    scrollY: () => 0,
    shown: () => true,
    fontFaces: () => (page === 'broken.html' ? BROKEN : STATUSES),
    resources: () => (page === 'broken.html' ? [] : RESOURCES),
  };
  const fonts = collect({ minTextPx: 12, minInputPx: 16, minLineHeight: 1.2 }, env);
  return `${ascii(JSON.stringify(fonts, null, 2))}\n`;
}

describe('the committed fixture answers', () => {
  for (const [name, page, html, width] of [
    ['wide', 'index.html', fixture, 1280],
    ['narrow', 'index.html', fixture, 375],
    ['broken', 'broken.html', broken, 1280],
  ] as const) {
    it(`tests/fixtures/${name}.json is the collector's answer for ${page} at ${width} px`, () => {
      const fresh = fontsAt(page, html, width);
      const file = resolve(OUT, `${name}.json`);
      if (process.env.FONTS_UPDATE_FIXTURES === '1') writeFileSync(file, fresh);
      expect(readFileSync(file, 'utf8') === fresh).toBe(true);
    });
  }
});
