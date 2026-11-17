// The collector as the component runs it: built into ../src/collect.js as the global
// `OutlineCollect`, evaluated in the plugin's isolated world of each view
// (`OutlineCollect.collect({...})`, src/page.rs).

import { collect } from './collect';
import type { PageOutline, Request } from './types';

/** One view's outline for `request`; a malformed request reads every section. */
export function run(request: unknown): PageOutline {
  const typed = request && typeof request === 'object' ? (request as Request) : {};
  return collect({
    sections: Array.isArray(typed.sections) ? typed.sections.filter((s) => typeof s === 'string') : undefined,
    custom: Array.isArray(typed.custom)
      ? typed.custom.filter(
          (entry) => entry && typeof entry.label === 'string' && typeof entry.selector === 'string',
        )
      : [],
  });
}
