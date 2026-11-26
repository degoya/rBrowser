// K9 (RB-010-09): an untrusted project runs no flows, plugins, snippets or mocks and applies no
// network overrides; the UI lists what is blocked. The project is a fresh copy of
// tests/fixtures/untrusted-project (its proxy and host rules point at 203.0.113.7, its headers
// add X-Untrusted, its plugin, flow, page script, snippet and mock exist) in a run whose trust
// store is empty. Checked as a client sees it: project_info, flow_run refused, the plugin listed as
// blocked (the list the Plugins settings show), the pages loading from the fixture server (no
// proxy, no host rule) without the header, and neither the page script nor the snippet run.

import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { needSession } from './lib/context.ts';

/** The untrusted project's start URL is on the fixture server's second port. */
const PAGE = 'http://127.0.0.1:18765/grid.html';

export const k09: Case = {
  id: 'K9',
  title: 'Untrusted project',
  threshold: 'nothing of the project runs or applies; blocked items listed',
  project: 'untrusted',
  needs: [],
  jobs: ['RB-010-09'],
  async run(ctx) {
    const session = needSession(ctx);
    const info = await session.mcp.value<{ trusted?: boolean }>('project_info');
    const flows = await session.mcp.value<{ trusted?: boolean }>('flow_list');
    const run = await session.mcp.call('flow_run', { name: 'login', target: 'all' });
    const refused = run.isError && run.text.includes('flows.untrusted');
    const plugins = await session.mcp.value<{ plugins?: { id?: string; state?: string }[] }>('plugins_list');
    const greeter = plugins.plugins?.find((plugin) => plugin.id === 'greeter');
    const since = Date.now();
    await session.navigateAll(PAGE);
    const titles = await session.evaluateAll<string>('document.title');
    const injected = titles.filter((entry) => entry.value === 'page script' || entry.value === 'snippet').length;
    const loads = ctx.server.requests().filter((request) => request.path === '/grid.html' && request.at >= since);
    const withHeader = loads.filter((request) => request.headers['x-untrusted'] !== undefined).length;
    const views = (await session.views()).length;
    return outcome(
      [
        measure('project_info trusted', info.trusted ?? null, 'false', info.trusted === false),
        measure('flow_list trusted', flows.trusted ?? null, 'false', flows.trusted === false),
        measure('flow_run', refused ? 'flows.untrusted' : run.text.slice(0, 80), 'refused (flows.untrusted)', refused),
        measure('project plugin', greeter?.state ?? 'not listed', 'blocked', greeter?.state === 'blocked'),
        measure('pages loaded (no proxy, no host rule)', `${loads.length} for ${views} views`, '>= views', loads.length >= views && views > 0),
        measure('requests with X-Untrusted', withHeader, '= 0', withHeader === 0),
        measure('page script or snippet ran', injected, '= 0', injected === 0),
      ],
      [
        'the trust dialog itself (the list of what is blocked) is a window no MCP tool reads: ' +
          'the plugin row "blocked" is the listed part; the dialog stays the Windows acceptance of RB-010-09',
      ],
    );
  },
};
