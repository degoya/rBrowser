// K10 (RB-000-05, RB-030-06): a CDP connection without a valid token is refused; with the token
// the shell UI is not a target. The endpoint is the MCP tool `cdp_endpoint`'s (inside WSL its
// route from WSL); the client is the one scripts/perf.sh uses, as Playwright's connectOverCDP
// would connect.

import { browserSocket } from '../../scripts/lib/perf-measure.mjs';

import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { leakedTargets, openCdp, pageTargets, socketOpens, statusWithout, targets } from './lib/cdp.ts';
import { needSession } from './lib/context.ts';

export const k10: Case = {
  id: 'K10',
  title: 'CDP proxy token and targets',
  threshold: 'refused without a valid token; no shell, DevTools or internal target with it',
  project: 'e2e',
  needs: [],
  jobs: ['RB-000-05', 'RB-030-06'],
  async run(ctx) {
    const session = needSession(ctx);
    const endpoint = await session.endpoint();
    const status = await statusWithout(endpoint.url);
    const socket = (await browserSocket(endpoint)) as string;
    // `url` of cdp_endpoint is the header form: its socket carries no token of its own.
    const withoutToken = await socketOpens(socket, {});
    const wrongToken = await socketOpens(socket, { Authorization: `Bearer ${'0'.repeat(64)}` });
    const cdp = await openCdp(endpoint);
    try {
      const infos = await targets(cdp);
      const leaked = leakedTargets(infos);
      for (const info of leaked) ctx.log(`visible: ${info.type} ${info.url}`);
      const pages = pageTargets(infos).length;
      const views = (await session.views()).filter((view) => view.engine === 'chromium').length;
      return outcome([
        measure('HTTP without a token', status, '401 or 403', status === 401 || status === 403),
        measure('WebSocket without a token', withoutToken ? 'opened' : 'refused', 'refused', !withoutToken),
        measure('WebSocket with a wrong token', wrongToken ? 'opened' : 'refused', 'refused', !wrongToken),
        measure('hidden targets visible', leaked.length, '= 0', leaked.length === 0),
        measure('page targets', `${pages} for ${views} Chromium views`, '= views', pages === views && views > 0),
      ]);
    } finally {
      cdp.close();
    }
  },
};
