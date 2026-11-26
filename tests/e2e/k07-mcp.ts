// K7 (RB-030-01, RB-030-02): `rbrowser.com mcp` from WSL - the run's own MCP client is exactly
// that - answers `navigate` with `target: all` with the final URL of every view. Without a running
// instance the error is understandable: a project nobody opened gets a JSON-RPC error that names
// what to do. With --native the client runs on Windows itself, no WSL in between.

import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { spawnMcp, windowsPath } from './lib/app.ts';
import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { needInstall, needSession, site } from './lib/context.ts';

/** What the offline error must say: that none runs and how to start one. */
const OFFLINE = [/no running rBrowser instance/i, /rbrowser open/i];

export const k07: Case = {
  id: 'K7',
  title: 'MCP navigate from WSL',
  threshold: 'navigate target all answers each view final URL; without an instance a clear error',
  project: 'e2e',
  needs: [],
  jobs: ['RB-030-01', 'RB-030-02'],
  async run(ctx) {
    const session = needSession(ctx);
    const install = needInstall(ctx);
    const url = site(ctx, '/sync.html?k7');
    const answer = await session.navigateAll(url);
    const total = (await session.views()).length;
    const finals = (answer.views ?? []).map((view) => view.url);
    const right = finals.filter((final) => final === url).length;

    // A project with a config and no rBrowser on it.
    const lonely = join(ctx.work, 'k7-no-instance');
    mkdirSync(join(lonely, '.rbrowser'), { recursive: true });
    writeFileSync(join(lonely, '.rbrowser', 'config.json'), '{ "version": 1 }\n');
    const { client, child } = spawnMcp(install, windowsPath(lonely, ctx.mode));
    let offline = '';
    try {
      await client.initialize();
      const call = await client.call('views_list', {}, 30_000);
      offline = call.isError ? call.text : `answered: ${call.text.slice(0, 200)}`;
    } catch (error) {
      offline = error instanceof Error ? error.message : String(error);
    } finally {
      child.stdin?.end();
      child.kill();
    }
    ctx.log(`without an instance: ${offline}`);
    const clear = OFFLINE.every((pattern) => pattern.test(offline));
    return outcome(
      [
        measure('final URL per view', `${right} of ${total}`, 'all', total > 0 && right === total && finals.length === total),
        measure('settled', answer.settled ?? null, 'true', answer.settled === true),
        measure('error without an instance', clear ? 'names the cause and rbrowser open' : offline.slice(0, 120), 'understandable', clear),
      ],
      [ctx.mode === 'wsl' ? 'MCP through rbrowser.com mcp from WSL (stdio over the interop)' : '--native: rbrowser.com mcp on Windows directly'],
    );
  },
};
