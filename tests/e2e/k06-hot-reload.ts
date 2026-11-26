// K6 (RB-030-05): saving a file in a WSL project reloads every view within 1 s; CSS without a
// full reload (the page's JS state kept). The project is the work folder's copy of
// tests/fixtures/e2e-project (inside WSL from WSL - the rbrowser-wsl watcher -, on Windows with
// --native - the local watcher); its www/ is served as /reload/. The page reports each reload to
// the fixture server's /log, stamped in the harness's clock like the moment the file was written.

import { writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { needSession, site } from './lib/context.ts';
import { stats, waitFor } from './lib/measure.ts';
import type { Context } from './lib/context.ts';

const BUDGET_MS = 1000;

/** The views' reports of `marker`, once each view sent one; their latencies from `at`. */
async function reloaded(ctx: Context, marker: string, at: number, views: number): Promise<number[]> {
  await waitFor(
    async () => {
      const seen = new Set(ctx.server.reports().filter((entry) => entry.marker === marker).map((entry) => entry.view));
      return seen.size >= views ? true : undefined;
    },
    { timeoutMs: 15_000, stepMs: 50, what: `${marker} from every view` },
  );
  const firsts = new Map<string, number>();
  for (const entry of ctx.server.reports()) {
    if (entry.marker === marker && entry.at >= at && !firsts.has(entry.view)) firsts.set(entry.view, entry.at);
  }
  return [...firsts.values()].map((seen) => seen - at);
}

export const k06: Case = {
  id: 'K6',
  title: 'Hot reload from the project',
  threshold: `every view reloaded <= ${BUDGET_MS} ms after the save; CSS without a full reload`,
  project: 'e2e',
  needs: [],
  jobs: ['RB-030-05'],
  async run(ctx) {
    const session = needSession(ctx);
    const www = join(ctx.work, 'e2e-project', 'www');
    await session.navigateAll(site(ctx, '/reload/reload.html'));
    const views = (await session.views()).length;
    const state = `kept-${Date.now()}`;
    await session.evaluateAll(`window.__e2eState = ${JSON.stringify(state)}`);

    const css = `css${Date.now()}`;
    const cssAt = Date.now();
    writeFileSync(join(www, 'reload.css'), `:root { --e2e-version: "${css}"; }\n#probe { color: #223; }\n`);
    const cssTimes = stats(await reloaded(ctx, `css-${css}`, cssAt, views));
    const kept = await session.evaluateAll<string | null>('window.__e2eState ?? null');
    const fullReloads = ctx.server.reports().filter((entry) => entry.marker.startsWith('load-') && entry.at >= cssAt).length;

    const js = `js${Date.now()}`;
    const jsAt = Date.now();
    writeFileSync(join(www, 'reload.js'), `window.E2E_JS_VERSION = '${js}';\n`);
    const jsTimes = stats(await reloaded(ctx, `load-${js}`, jsAt, views));

    const notes = [ctx.mode === 'wsl' ? 'the project lives in WSL (rbrowser-wsl watcher)' : '--native: the project lives on Windows (local watcher)'];
    return outcome(
      [
        measure('CSS reload (slowest view)', cssTimes.maxMs, `<= ${BUDGET_MS} ms`, cssTimes.count === views && cssTimes.maxMs <= BUDGET_MS, 'ms'),
        measure('JS state kept after CSS', `${kept.filter((entry) => entry.value === state).length} of ${views}`, 'all',
          kept.length === views && kept.every((entry) => entry.value === state)),
        measure('full reloads on the CSS change', fullReloads, '= 0', fullReloads === 0),
        measure('JS reload (slowest view)', jsTimes.maxMs, `<= ${BUDGET_MS} ms`, jsTimes.count === views && jsTimes.maxMs <= BUDGET_MS, 'ms'),
      ],
      notes,
    );
  },
};
