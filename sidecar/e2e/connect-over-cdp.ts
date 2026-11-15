// The Node half of RB-040-06's end-to-end test: Playwright's `chromium.connectOverCDP` against
// rBrowser's CDP proxy. The Rust half (crates/rbrowser/src/cdp_proxy/playwright_tests.rs) runs the
// proxy in front of a scripted browser, writes the answer of `cdp_endpoint` to the file named by
// RBROWSER_CDP_ENDPOINT and starts
//
//   node e2e/connect-over-cdp.ts <url of the page to click> <url of the page to navigate> <new url>
//
// This script connects without and with wrong tokens, then with the header, the path and the
// ws:// form, takes one step per view, and prints one JSON report line; the Rust test judges the
// report and what reached the browser. Nothing printed here carries a token.

import { readFileSync } from 'node:fs';

import { type Browser, chromium } from 'playwright-core';

import { endpointFromAnswer } from '../../examples/playwright/endpoint.ts';

const TIMEOUT = 10_000;
const WRONG = 'f'.repeat(64);

/** A message's first line, any 64-digit hex string (a token) blotted out. */
function firstLine(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return (message.split('\n')[0] ?? '').replace(/[0-9a-f]{64}/g, '<token>');
}

function field(answer: unknown, name: string): string {
  const value = (answer as Record<string, unknown>)[name];
  if (typeof value !== 'string') {
    throw new Error(`the cdp_endpoint answer has no ${name}`);
  }
  return value;
}

/** Whether the proxy refuses a connection to `url` with `headers`. */
async function refused(url: string, headers?: Record<string, string>): Promise<boolean> {
  try {
    const browser = await chromium.connectOverCDP(url, { headers, timeout: TIMEOUT });
    await browser.close();
    return false;
  } catch {
    return true;
  }
}

/** The contexts and the URLs of the pages a client sees. */
function seen(browser: Browser): { contexts: number; pages: string[] } {
  const contexts = browser.contexts();
  const pages = contexts.flatMap((context) => context.pages().map((page) => page.url()));
  return { contexts: contexts.length, pages: pages.sort() };
}

async function main(): Promise<Record<string, unknown>> {
  const [clickUrl, gotoUrl, nextUrl] = process.argv.slice(2);
  if (clickUrl === undefined || gotoUrl === undefined || nextUrl === undefined) {
    throw new Error('usage: connect-over-cdp.ts <click page url> <goto page url> <new url>');
  }
  const answer: unknown = JSON.parse(readFileSync(process.env.RBROWSER_CDP_ENDPOINT ?? '', 'utf8'));
  const endpoint = endpointFromAnswer(answer, false);
  const base = field(answer, 'url');
  const report: Record<string, unknown> = {};

  report.refused = {
    none: await refused(base),
    wrongHeader: await refused(base, { Authorization: `Bearer ${WRONG}` }),
    wrongPath: await refused(`${base}/t/${WRONG}`),
  };

  const viaPath = await chromium.connectOverCDP(field(answer, 'urlWithToken'), { timeout: TIMEOUT });
  report.path = seen(viaPath);
  await viaPath.close();

  const viaSocket = await chromium.connectOverCDP(field(answer, 'webSocketDebuggerUrl'), {
    headers: endpoint.headers,
    timeout: TIMEOUT,
  });
  report.ws = seen(viaSocket);
  await viaSocket.close();

  const browser = await chromium.connectOverCDP(endpoint.url, { headers: endpoint.headers, timeout: TIMEOUT });
  try {
    report.header = seen(browser);
    const context = browser.contexts()[0];
    const pages = context?.pages() ?? [];
    const clicked = pages.find((page) => page.url() === clickUrl);
    const navigated = pages.find((page) => page.url() === gotoUrl);
    if (context === undefined || clicked === undefined || navigated === undefined) {
      throw new Error('a view the test expects is no page');
    }
    // rBrowser owns the size: answered, never sent to the view.
    await clicked.setViewportSize({ width: 300, height: 400 });
    report.viewportSet = true;
    // CSS px of the page; the proxy scales them to the view.
    await clicked.mouse.click(100, 60);
    report.clicked = true;
    await navigated.goto(nextUrl, { timeout: TIMEOUT });
    report.navigated = navigated.url();
    report.clickedUrl = clicked.url();
    try {
      await context.newCDPSession(clicked);
      report.cdpSession = 'opened';
    } catch (error) {
      report.cdpSession = firstLine(error);
    }
  } finally {
    await browser.close();
  }
  return report;
}

try {
  console.log(JSON.stringify(await main()));
} catch (error) {
  console.log(JSON.stringify({ error: firstLine(error) }));
  process.exitCode = 1;
}
