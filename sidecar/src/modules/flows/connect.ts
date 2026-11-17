// How the flow worker connects to a view (RB-040-07): `connectOverCDP` with the endpoint's token
// header and `noDefaults`, so Playwright leaves the shared default context as rBrowser set it -
// no browser-wide download behaviour into Playwright's temp folder, no font families, no focus
// emulation on the views it attaches to (RB-040-06).

import type { Endpoint } from './result.ts';

/** The most a connect may take; less when the flow's timeout is shorter. */
export const CONNECT_TIMEOUT_MS = 15_000;

/** The options of `chromium.connectOverCDP(endpoint.url, ...)`. */
export function connectOptions(
  endpoint: Endpoint,
  timeoutMs: number,
): { headers: Record<string, string>; timeout: number; noDefaults: true } {
  return { headers: endpoint.headers, timeout: Math.min(timeoutMs, CONNECT_TIMEOUT_MS), noDefaults: true };
}
