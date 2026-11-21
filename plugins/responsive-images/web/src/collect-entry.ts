// The collector as the component runs it: built into ../src/collect.js as the global
// `ResponsiveImagesCollect`, evaluated in the plugin's isolated world of each view
// (`ResponsiveImagesCollect.run({...})`, src/page.rs); `evaluate` awaits the promise.

import { collect, type Env, realEnv } from './collect';
import type { PageImages, Request } from './types';

/** One view's images for `request`; a malformed request takes the defaults. */
export function run(request: unknown, env: Env = realEnv): Promise<PageImages> {
  const typed = request && typeof request === 'object' ? (request as Request) : {};
  return collect(typed, env);
}
