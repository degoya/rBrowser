// A real Lighthouse run in a real headless browser against a page of a local server - opt-in
// (`RB_LIGHTHOUSE_REAL=1 npm test`), so `npm test` stays hermetic and fast. It needs a browser
// browser.ts finds (Playwright's headless shell, Chrome, Edge or CHROME_PATH).

import { readFileSync, readdirSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import type { AddressInfo } from 'node:net';

import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import { realLighthouseDeps } from './index.ts';
import { readStart } from './params.ts';
import { systemBrowser } from './real.ts';

const PAGE = `<!doctype html><html><head><title>rBrowser Lighthouse fixture</title>
<meta name="viewport" content="width=device-width"></head>
<body><h1>Fixture</h1><img src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" width="10" height="10">
<p style="color:#bbb">low contrast</p></body></html>`;

const enabled = process.env.RB_LIGHTHOUSE_REAL === '1' && systemBrowser() !== null;

/** How many processes descend from this one (Linux: `/proc`). */
function descendants(): number {
  const parents = new Map<number, number>();
  for (const name of readdirSync('/proc').filter((entry) => /^\d+$/.test(entry))) {
    try {
      const stat = readFileSync(`/proc/${name}/stat`, 'utf8');
      parents.set(Number(name), Number(stat.slice(stat.lastIndexOf(')') + 2).split(' ')[1]));
    } catch {
      // Ended between the listing and the read.
    }
  }
  const ours = new Set([process.pid]);
  for (let grew = true; grew; ) {
    grew = false;
    for (const [pid, parent] of parents) {
      if (!ours.has(pid) && ours.has(parent)) {
        ours.add(pid);
        grew = true;
      }
    }
  }
  return ours.size - 1;
}

describe.skipIf(!enabled)('a real Lighthouse run', () => {
  const server = createServer((_request, response) => {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end(PAGE);
  });
  let url = '';

  beforeAll(async () => {
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
    url = `http://127.0.0.1:${(server.address() as AddressInfo).port}/`;
  });

  afterAll(() => {
    server.close();
  });

  for (const [label, width, mobile] of [
    ['mobile', 375, true],
    ['desktop', 1280, false],
  ] as const) {
    it(`measures the ${label} breakpoint with its own viewport`, { timeout: 120_000 }, async () => {
      const request = readStart({
        url,
        label,
        settings: {
          formFactor: mobile ? 'mobile' : 'desktop',
          screenEmulation: { width, height: 800, deviceScaleFactor: mobile ? 2 : 1, mobile },
          throttlingMethod: 'simulate',
          onlyCategories: ['accessibility', 'seo'],
        },
      });
      const output = await realLighthouseDeps().runner(request, new AbortController().signal);
      const lhr = JSON.parse(output.json) as {
        configSettings: { screenEmulation: { width: number }; formFactor: string };
        categories: Record<string, { score: number | null }>;
        audits: Record<string, { score: number | null }>;
      };
      expect(lhr.configSettings.screenEmulation.width).toBe(width);
      expect(lhr.configSettings.formFactor).toBe(label);
      expect(Object.keys(lhr.categories).sort()).toEqual(['accessibility', 'seo']);
      expect(lhr.audits['color-contrast']?.score).toBe(0);
      expect(output.html).toContain('<html');
    });
  }

  // RB-080-67: two runs in a row leave as many processes and profiles as there were before.
  it.skipIf(process.platform !== 'linux')('leaves no process behind after two runs', { timeout: 240_000 }, async () => {
    const profiles = () => readdirSync(tmpdir()).filter((name) => name.startsWith('rbrowser-lighthouse-')).length;
    const before = descendants();
    const profilesBefore = profiles();
    for (const label of ['first', 'second']) {
      const request = readStart({
        url,
        label,
        settings: {
          formFactor: 'mobile',
          screenEmulation: { width: 375, height: 800, deviceScaleFactor: 2, mobile: true },
          throttlingMethod: 'simulate',
          onlyCategories: ['seo'],
        },
      });
      await realLighthouseDeps().runner(request, new AbortController().signal);
    }
    expect(descendants()).toBe(before);
    expect(profiles()).toBe(profilesBefore);
  });
});
