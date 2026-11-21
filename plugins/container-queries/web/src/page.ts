// The page script of the container-queries plugin (manifest [[page_scripts]], bundled into
// ../page/containers.js by scripts/bundle.mjs). It runs in the plugin's isolated world of every
// view, top frame only (src/start.ts).

import { liveEnv } from './collect';
import { start } from './start';
import type { RbPlugin } from './types';

declare const rbPlugin: RbPlugin | undefined;

if (typeof rbPlugin !== 'undefined' && window === window.top) {
  start(rbPlugin, document, window, liveEnv(window));
}
