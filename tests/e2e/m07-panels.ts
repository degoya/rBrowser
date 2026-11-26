// Milestone 0.7 (M7 + M8, docs/roadmap.md "0.7 - Panels, Overlays & Tools") as a smoke: the
// plugins of the milestone load in the package (state `active` in `plugins_list`, the list the
// Plugins settings show) and the project bookmarks answer. What each plugin does is its job's own
// acceptance (tests/e2e/README.md, the milestone table).

import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { needSession } from './lib/context.ts';

const PLUGINS = [
  'storage-panel',
  'network-panel',
  'outline',
  'snippets',
  'json-xml-viewer',
  'simulators',
  'image-overlay',
  'color-picker',
  'form-autofill',
  'fonts',
];

export const m07: Case = {
  id: 'M7',
  title: 'Milestone 0.7 plugins load',
  threshold: 'every 0.7 plugin active; bookmarks_list answers',
  project: 'e2e',
  needs: [],
  jobs: ['RB-070-01', 'RB-070-06', 'RB-070-13'],
  async run(ctx) {
    const session = needSession(ctx);
    const list = await session.mcp.value<{ plugins?: { id?: string; state?: string }[] }>('plugins_list');
    const active = new Set((list.plugins ?? []).filter((plugin) => plugin.state === 'active').map((plugin) => plugin.id));
    const missing = PLUGINS.filter((id) => !active.has(id));
    for (const id of missing) ctx.log(`not active: ${id}`);
    const bookmarks = await session.mcp.call('bookmarks_list');
    return outcome([
      measure('0.7 plugins active', `${PLUGINS.length - missing.length} of ${PLUGINS.length}`, 'all', missing.length === 0),
      measure('bookmarks_list', bookmarks.isError ? 'error' : 'answered', 'answered', !bookmarks.isError),
    ]);
  },
};
