// The frameworks each view's page runs, as the shell offers their DevTools (RB-080-24): rb-ipc
// `frameworks::ViewFrameworks` with rb-sync `FrameworkInfo`, re-exported by `protocol.ts`. A
// framework is listed only while its setting (`frameworkDevtools.vue` / `.react`) is not off.
import type { ViewId } from './protocol';

/** The frameworks whose DevTools rBrowser brings. */
export type Framework = 'vue' | 'react';

/** What the core's probe found of one framework in a page. */
export interface FrameworkInfo {
  /** The framework's version, when it says. */
  version?: string;
  /** rBrowser's DevTools hook is in the document; without it the view needs one reload. */
  hook: boolean;
  /** The framework registered with a hook: false for a Vue production build without
   *  `__VUE_PROD_DEVTOOLS__`, or React loaded before any hook. */
  devtools: boolean;
}

/** What a view's page runs of Vue and React. */
export interface ViewFrameworks {
  view: ViewId;
  vue?: FrameworkInfo;
  react?: FrameworkInfo;
}
