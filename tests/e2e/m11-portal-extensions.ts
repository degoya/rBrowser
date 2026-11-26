// Milestone 0.8, portal, extensions and extras (M11, docs/roadmap.md "0.8"): the portal answers
// its state (off by default; pairing needs a person with the QR code and a device on the LAN - a
// documented measurement, tests/e2e/README.md), the Chrome extensions answer theirs, and the
// extra plugins of the milestone load. MCP can neither start the portal nor load an unpacked
// extension: both are a person's decision.

import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { needSession } from './lib/context.ts';

const PLUGINS = ['coverage', 'pwa-check', 'error-report', 'seo'];

export const m11: Case = {
  id: 'M11',
  title: 'Milestone 0.8 portal, extensions, extras',
  threshold: 'portal_devices and extension_list answer; the portal off by default; the extra plugins active',
  project: 'e2e',
  needs: [],
  jobs: ['RB-080-05', 'RB-080-06', 'RB-080-23', 'RB-080-07', 'RB-080-08', 'RB-080-09'],
  async run(ctx) {
    const session = needSession(ctx);
    const portal = await session.mcp.call('portal_devices');
    const running = (portal.value as { running?: boolean } | null)?.running;
    const extensions = await session.mcp.call('extension_list');
    ctx.log(`portal: ${portal.text.slice(0, 300)}`);
    const list = await session.mcp.value<{ plugins?: { id?: string; state?: string }[] }>('plugins_list');
    const active = new Set((list.plugins ?? []).filter((plugin) => plugin.state === 'active').map((plugin) => plugin.id));
    const missing = PLUGINS.filter((id) => !active.has(id));
    for (const id of missing) ctx.log(`not active: ${id}`);
    return outcome([
      measure('portal_devices', portal.isError ? 'error' : 'answered', 'answered', !portal.isError),
      measure('portal running', running ?? null, 'false (off by default)', running !== true),
      measure('extension_list', extensions.isError ? 'error' : 'answered', 'answered', !extensions.isError),
      measure('0.8 extra plugins active', `${PLUGINS.length - missing.length} of ${PLUGINS.length}`, 'all', missing.length === 0),
    ]);
  },
};
