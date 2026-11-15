// Browsersync detection (RB-020-06). Browsersync's ghost mode replays clicks, scrolls and forms
// between the browsers it serves, so with rBrowser's own sync on every input would arrive twice.
// The agent runs in an isolated world and cannot read the page's `___browserSync___`; it sees
// the DOM and the resource timing, which are shared: the snippet Browsersync injects after
// `<body>`, its client script, a request to its socket. Ghost mode is Browsersync's default,
// so the client's presence counts; the core decides (`sync.detectBrowsersync`).

/** The snippet Browsersync injects, and its client script. */
const SELECTORS = ['script#__bs_script__', 'script[src*="/browser-sync/browser-sync-client"]'];
/** The path of Browsersync's socket. */
const SOCKET_PATH = '/browser-sync/socket.io/';

/** Whether `doc` carries Browsersync's client, or `perf` lists a request to its socket. */
export function hasBrowsersync(doc: Document, perf?: Pick<Performance, 'getEntriesByType'>): boolean {
  if (SELECTORS.some((selector) => doc.querySelector(selector) !== null)) return true;
  try {
    return (perf?.getEntriesByType('resource') ?? []).some((entry) => entry.name.includes(SOCKET_PATH));
  } catch {
    return false;
  }
}

/** Calls `found` once when Browsersync's client is in `win`'s document: at once when it is
 *  parsed, else at `DOMContentLoaded` or `load`. Returns the function that stops watching. */
export function watchBrowsersync(win: Window, found: () => void): () => void {
  let done = false;
  const stop = (): void => {
    win.document.removeEventListener('DOMContentLoaded', check);
    win.removeEventListener('load', check);
  };
  function check(): void {
    if (done || !hasBrowsersync(win.document, win.performance)) return;
    done = true;
    stop();
    found();
  }
  win.document.addEventListener('DOMContentLoaded', check);
  win.addEventListener('load', check);
  if (win.document.readyState !== 'loading') check();
  return stop;
}
