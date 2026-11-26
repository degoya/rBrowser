// Milestone 0.9 (M12, docs/roadmap.md "0.9 - Public Beta"): the package under test is the
// checkout's version, its update state answers (self-update is off in the run's settings), and
// the bundled plugins load. Signing, the channels and the public mirror are the release's steps
// (tests/e2e/README.md, the milestone table).

import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { measure, outcome } from './lib/case.ts';
import { workspaceVersion } from './lib/checks.ts';
import type { Case } from './lib/case.ts';
import { needSession } from './lib/context.ts';

export const m12: Case = {
  id: 'M12',
  title: 'Milestone 0.9 package',
  threshold: 'the checkout version; update_status answers; no bundled plugin in error',
  project: 'e2e',
  needs: [],
  jobs: ['RB-099-01', 'RB-090-14'],
  async run(ctx) {
    const session = needSession(ctx);
    const wanted = workspaceVersion(readFileSync(join(ctx.root, 'Cargo.toml'), 'utf8'));
    const info = await session.mcp.value<{ version?: string }>('app_info');
    const update = await session.mcp.call('update_status');
    const list = await session.mcp.value<{ plugins?: { id?: string; state?: string; source?: string }[] }>('plugins_list');
    const broken = (list.plugins ?? []).filter((plugin) => plugin.source === 'bundled' && plugin.state === 'error');
    for (const plugin of broken) ctx.log(`bundled plugin in error: ${plugin.id ?? '?'}`);
    return outcome([
      measure('version', info.version ?? null, wanted, info.version === wanted),
      measure('update_status', update.isError ? 'error' : 'answered', 'answered', !update.isError),
      measure('bundled plugins in error', broken.length, '= 0', broken.length === 0),
    ]);
  },
};
