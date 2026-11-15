// Playwright against rBrowser (RB-040-06): connect to the CDP proxy, then one step per view -
// which view it is and a screenshot at its breakpoint.
//
//   npm install
//   RBROWSER_CDP_URL=http://127.0.0.1:<port> RBROWSER_CDP_TOKEN=<token> npm start
//   RBROWSER_CDP_ENDPOINT=endpoint.json npm start      # the saved answer of cdp_endpoint
//
// The endpoint comes from the environment (endpoint.ts), never from this file. Every Chromium
// view of the grid is one page of the one context; the shell, the settings window and DevTools
// are not. rBrowser owns each view's size: page.setViewportSize has no effect - pick the view.

import { readFileSync } from 'node:fs';

import { chromium } from 'playwright-core';

import { resolveEndpoint } from './endpoint.ts';

/** What the page API (`window.__rbrowser`, setting `pageApi.enabled`) tells about a view. */
interface View {
  id?: string;
  label?: string;
  width: number;
  height: number;
}

const endpoint = resolveEndpoint(process.env, (path) => readFileSync(path, 'utf8'));
// noDefaults: Playwright leaves the shared default context as rBrowser set it (no browser-wide
// download behaviour, fonts or focus emulation of its own).
const browser = await chromium.connectOverCDP(endpoint.url, { headers: endpoint.headers, noDefaults: true });
try {
  const pages = browser.contexts().flatMap((context) => context.pages());
  console.log(`${pages.length} view(s)`);
  for (const [index, page] of pages.entries()) {
    const view = await page.evaluate((): View => {
      const scope = globalThis as unknown as {
        __rbrowser?: { id: string; label: string };
        innerWidth: number;
        innerHeight: number;
      };
      const api = scope.__rbrowser;
      return { id: api?.id, label: api?.label, width: scope.innerWidth, height: scope.innerHeight };
    });
    const name = view.label ?? `${view.width}px`;
    const file = `view-${index + 1}-${name.replace(/[^\w.-]+/g, '_')}.png`;
    await page.screenshot({ path: file });
    console.log(`${name} (${view.id ?? 'no page API'}): ${page.url()} at ${view.width}x${view.height} -> ${file}`);
  }
} finally {
  // Disconnects only: rBrowser and its views stay open (Browser.close is refused).
  await browser.close();
}
