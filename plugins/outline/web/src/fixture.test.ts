// The collector's answers for the acceptance page tests/fixtures/site/outline/ in a wide and a
// narrow view, committed as plugins/outline/tests/fixtures/{wide,narrow}.json: the component's
// native tests (tests/fixture.rs) take them as the views' pages, so the page, the collector and the
// findings are tested end to end without a browser. `OUTLINE_UPDATE_FIXTURES=1 npm test` writes
// them; otherwise this fails when they are stale.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import fixture from '../../../../tests/fixtures/site/outline/index.html?raw';
import { run } from './collect-entry';
import type { Box } from './types';
import type { Env } from './visible';

const OUT = resolve(dirname(fileURLToPath(import.meta.url)), '../../tests/fixtures');
const URL = 'http://localhost:8080/outline/index.html';
const flat: Env = { rect: (): Box => [10, 10, 10, 10], pageWidth: () => 1280 };

function outlineAt(width: number): string {
  const happy = (window as unknown as { happyDOM: { setURL(url: string): void; setViewport(v: { width: number; height: number }): void } }).happyDOM;
  happy.setURL(URL);
  happy.setViewport({ width, height: 800 });
  document.documentElement.setAttribute('lang', 'en');
  document.head.innerHTML = `<style>${/<style>([\s\S]*?)<\/style>/.exec(fixture)?.[1] ?? ''}</style>`;
  document.body.innerHTML = (/<body>([\s\S]*)<\/body>/.exec(fixture)?.[1] ?? '').replace(/<script>[\s\S]*?<\/script>/g, '');
  const outline = run({ custom: [{ label: 'CMS blocks', selector: '[data-cms-block]' }] });
  // The collector's real geometry is the page's; here every box is the same.
  return `${JSON.stringify({ ...outline, url: URL }, null, 2)}\n`;
}

describe('the committed fixture outlines', () => {
  for (const [name, width] of [
    ['wide', 1280],
    ['narrow', 375],
  ] as const) {
    it(`tests/fixtures/${name}.json is the collector's answer at ${width} px`, () => {
      const fresh = outlineAt(width);
      const file = resolve(OUT, `${name}.json`);
      if (process.env.OUTLINE_UPDATE_FIXTURES === '1') writeFileSync(file, fresh);
      expect(readFileSync(file, 'utf8') === fresh).toBe(true);
    });
  }
});
