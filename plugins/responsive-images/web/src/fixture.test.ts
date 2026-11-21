// @vitest-environment node
// The collector's answers of the acceptance fixture (fixtures.ts), one per breakpoint, are the
// files the Rust fixture test hands the component as each view's `evaluate` answer
// (../tests/fixtures/<breakpoint>.json). They are what analyze.ts makes of the fixture; with
// RB_UPDATE_FIXTURES=1 the test writes them.
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { analyze, DEFAULTS } from './analyze';
import { BREAKPOINTS, fixtureImages, PAGE_URL } from './fixtures';
import type { PageImages } from './types';

const dir = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'tests', 'fixtures');

describe('the Rust fixtures', () => {
  for (const breakpoint of BREAKPOINTS) {
    it(`${breakpoint.id}.json is the collector's answer at DPR ${breakpoint.dpr}`, () => {
      const answer: PageImages = {
        url: PAGE_URL,
        dpr: breakpoint.dpr,
        viewport: { width: breakpoint.width, height: breakpoint.height },
        images: analyze(fixtureImages(breakpoint), { ...DEFAULTS, dpr: breakpoint.dpr }),
        more: 0,
        lcpKnown: true,
      };
      const text = `${JSON.stringify(answer, null, 2)}\n`;
      const file = join(dir, `${breakpoint.id}.json`);
      if (process.env.RB_UPDATE_FIXTURES === '1') writeFileSync(file, text);
      expect(readFileSync(file, 'utf8')).toBe(text);
    });
  }
});
