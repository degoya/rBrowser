// The follow of a SPA route (RB-020-06): the core's `navigate` command moves the follower's
// document to the leader's route without a reload, so the page keeps its state. A route change
// is `history.pushState` followed by a `popstate`, which the page's router (Vue Router, React
// Router, ...) takes as a history step and renders; a fragment of the same page sets the hash,
// so the browser scrolls to the anchor and fires `hashchange` itself. Only URLs of the
// document's own origin are taken - anything else would need a load, the core's job.

import type { CommandResult } from './protocol';

function error(message: string): CommandResult {
  return { status: 'error', message };
}

/** Moves `win`'s document to `url` through the history API; never throws. */
export function followRoute(win: Window, url: unknown): CommandResult {
  if (typeof url !== 'string') return error('invalid url');
  let next: URL;
  let here: URL;
  try {
    here = new URL(win.location.href);
    next = new URL(url, here);
  } catch {
    return error('invalid url');
  }
  if (next.origin === 'null' || next.origin !== here.origin) return error('another origin');
  if (next.href === here.href) return { status: 'done' };
  try {
    const samePage = next.pathname === here.pathname && next.search === here.search;
    if (samePage && next.hash !== '') {
      win.location.hash = next.hash;
      return { status: 'done' };
    }
    win.history.pushState(null, '', next.href);
    win.dispatchEvent(new PopStateEvent('popstate', { state: null }));
    if (next.hash !== here.hash) {
      win.dispatchEvent(new HashChangeEvent('hashchange', { oldURL: here.href, newURL: next.href }));
    }
    return { status: 'done' };
  } catch (cause) {
    return error(String(cause));
  }
}
