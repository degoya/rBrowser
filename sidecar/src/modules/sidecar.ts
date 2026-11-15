// `sidecar.ping` (the health check rb-sidecar sends after each start) and `sidecar.versions`
// (what the diagnostics ZIP and About show).

import type { Router } from '../router.ts';
import type { BrowserState } from './playwright.ts';

export interface VersionsDeps {
  playwrightVersion(): string;
  browsers(): BrowserState[];
}

export function registerSidecar(router: Router, deps: VersionsDeps): void {
  router.register('sidecar.ping', () => ({ pong: true }));
  router.register('sidecar.versions', () => ({
    node: process.versions.node,
    playwright: deps.playwrightVersion(),
    browsers: deps.browsers(),
    methods: router.methods(),
  }));
}
