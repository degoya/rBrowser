// The framework probe of a view's main world (RB-080-24): what a page runs of Vue and React.
// The agent sits in an isolated world and sees neither the frameworks' globals nor the expando
// properties they put on DOM nodes, so the core injects this probe into the main world (no world
// name) whenever Framework DevTools are not off. It defines no global, keeps nothing, and
// dispatches one DOM event on `document` whose `detail` is JSON text - the agent in the isolated
// world listens (`../framework.ts`) and reports it to the core.
//
// Vue 3 marks its mount container with `data-v-app` in every build and, in a development build
// or a production build with `__VUE_PROD_DEVTOOLS__`, hangs the app on it (`__vue_app__`);
// rBrowser's Vue hook (`vue-devtools.ts`) records the apps Vue announces. React registers its
// renderer with `__REACT_DEVTOOLS_GLOBAL_HOOK__` when the hook is there before React loads;
// without a hook it leaves only `__reactFiber$<key>` on its elements, which the probe scans.

import type { FrameworkInfo, FrameworkReport } from '../protocol';
import { FRAMEWORK_EVENT, HOOK_MARKER } from '../protocol';
import { isObject } from '../../../plugins/web-common/src/is-object';

/** How many elements the React fiber scan looks at (a key lookup each; cheap, but bounded). */
export const MAX_SCANNED_ELEMENTS = 3000;
/** When the probe looks again after `load`, in ms: SPAs mount after their chunks arrive. */
const RECHECK_MS = [1000, 3000, 8000];
/** The longest version string taken over. */
const MAX_VERSION = 64;

type Scope = Window & Record<string, unknown>;


function versionOf(value: unknown): string | undefined {
  return isObject(value) && typeof value.version === 'string' && value.version.length <= MAX_VERSION
    ? value.version
    : undefined;
}

function info(hook: boolean, devtools: boolean, version?: string): FrameworkInfo {
  return version === undefined ? { hook, devtools } : { version, hook, devtools };
}

function detectVue(win: Scope): FrameworkInfo | undefined {
  const hook = win.__VUE_DEVTOOLS_GLOBAL_HOOK__;
  const ours = isObject(hook) && hook[HOOK_MARKER] === true;
  if (isObject(hook) && Array.isArray(hook.apps) && hook.apps.length > 0) {
    return info(ours, true, versionOf(hook.apps[0]));
  }
  const containers = win.document.querySelectorAll('[data-v-app]');
  for (const container of containers) {
    const app = (container as Element & { __vue_app__?: unknown }).__vue_app__;
    if (isObject(app)) return info(ours, true, versionOf(app));
  }
  if (containers.length > 0) return info(ours, false);
  if (win.__VUE__ === true) return info(ours, false);
  return undefined;
}

/** Whether `element` carries a React fiber or root container key. */
function hasReactKey(element: Element): boolean {
  for (const key of Object.keys(element)) {
    if (key.startsWith('__reactFiber$') || key.startsWith('__reactContainer$')) return true;
  }
  return false;
}

function detectReact(win: Scope): FrameworkInfo | undefined {
  const hook = win.__REACT_DEVTOOLS_GLOBAL_HOOK__;
  const ours = isObject(hook) && hook[HOOK_MARKER] === true;
  if (isObject(hook) && hook.renderers instanceof Map && hook.renderers.size > 0) {
    const first: unknown = hook.renderers.values().next().value;
    return info(ours, true, versionOf(first));
  }
  const body = win.document.body;
  if (!body) return undefined;
  if (hasReactKey(body)) return info(ours, false);
  const elements = body.getElementsByTagName('*');
  const count = Math.min(elements.length, MAX_SCANNED_ELEMENTS);
  for (let i = 0; i < count; i += 1) {
    const element = elements[i];
    if (element && hasReactKey(element)) return info(ours, false);
  }
  return undefined;
}

/** What `win`'s document runs of Vue and React; never throws. */
export function detectFrameworks(win: Window): FrameworkReport {
  const scope = win as Scope;
  const report: FrameworkReport = {};
  try {
    const vue = detectVue(scope);
    if (vue) report.vue = vue;
  } catch {
    // A page whose hook throws: counted as no Vue.
  }
  try {
    const react = detectReact(scope);
    if (react) report.react = react;
  } catch {
    // As above.
  }
  return report;
}

/** Runs the probe in `win`'s top document: at `DOMContentLoaded`, at `load` and a few times
 *  after it, dispatching `FRAMEWORK_EVENT` whenever the finding changed. Returns the function
 *  that stops it. A child frame gets nothing. */
export function installProbe(win: Window): () => void {
  if (win !== win.top) return () => undefined;
  // Taken before any page script runs, so a page that replaces them changes nothing here.
  const doc = win.document;
  const CustomEventCtor = (win as Window & typeof globalThis).CustomEvent;
  const stringify = JSON.stringify;
  const setTimeout = win.setTimeout.bind(win);
  const clearTimeout = win.clearTimeout.bind(win);
  let last = '';
  let stopped = false;
  const timers: number[] = [];

  const check = (): void => {
    if (stopped) return;
    const text = stringify(detectFrameworks(win));
    if (text === last) return;
    last = text;
    doc.dispatchEvent(new CustomEventCtor(FRAMEWORK_EVENT, { detail: text }));
  };
  const onLoad = (): void => {
    check();
    for (const ms of RECHECK_MS) timers.push(setTimeout(check, ms));
  };
  doc.addEventListener('DOMContentLoaded', check);
  win.addEventListener('load', onLoad);
  if (doc.readyState === 'complete') onLoad();
  return () => {
    stopped = true;
    doc.removeEventListener('DOMContentLoaded', check);
    win.removeEventListener('load', onLoad);
    for (const timer of timers) clearTimeout(timer);
  };
}
