// The framework detection on the agent's side (RB-080-24). The probe the core injects into the
// main world (`framework/probe.ts`) dispatches `FRAMEWORK_EVENT` on `document` with the JSON
// text of a `FrameworkReport`; the agent checks it (`framework/report.ts`, the one validator the
// panels share) and reports it to the core, which offers the DevTools panel and, in `auto`
// mode, puts the hook into the next document.

import type { FrameworkReport } from './protocol';
import { FRAMEWORK_EVENT } from './protocol';
import { isFrameworkReport } from './framework/report';

/** Calls `found` with every valid report the probe dispatches in `win`'s document. Returns the
 *  function that stops watching. */
export function watchFrameworks(win: Window, found: (report: FrameworkReport) => void): () => void {
  const doc = win.document;
  const listener = (event: Event): void => {
    const detail: unknown = (event as CustomEvent<unknown>).detail;
    if (typeof detail !== 'string') return;
    let report: unknown;
    try {
      report = JSON.parse(detail);
    } catch {
      return;
    }
    if (!isFrameworkReport(report)) return;
    const clean: FrameworkReport = {};
    if (report.vue) clean.vue = report.vue;
    if (report.react) clean.react = report.react;
    found(clean);
  };
  doc.addEventListener(FRAMEWORK_EVENT, listener);
  return () => doc.removeEventListener(FRAMEWORK_EVENT, listener);
}
