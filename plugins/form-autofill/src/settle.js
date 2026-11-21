// The settle runner of the form autofill (RB-094-22): src/fill.rs runs it as `(<this function>)()`
// and hands the `settled` it answers to src/page.js. `settled(steps, settle)` runs the steps of an
// `apply` - a generator that yields after each control field it changed - and at each yield waits
// until the document has had no DOM change for `quiet` milliseconds (a MutationObserver), at the
// latest `cap` milliseconds, all waits of one apply together at most `budget` milliseconds. A
// framework that renders the fields a select brings after its `change` has rendered them before
// the next field is set, and src/fill.rs reads the fields again for the next pass.
// @ts-check
/** @returns {(steps: Iterator<unknown>, settle: { quiet?: unknown, cap?: unknown, budget?: unknown }) => Promise<void>} */
() => {
  'use strict';
  /** The longest any wait may be, whatever the request says (an evaluate may take 30 s). */
  const MAX_MS = 20000;
  /** @param {unknown} value */
  const ms = (value) => Math.min(MAX_MS, Math.max(0, Math.floor(Number(value) || 0)));

  /** Resolves once the document stayed unchanged for `quiet` ms, at the latest after `cap` ms. @param {number} quiet @param {number} cap */
  function settle(quiet, cap) {
    return new Promise((resolve) => {
      /** @type {ReturnType<typeof setTimeout> | undefined} */
      let timer;
      /** @type {ReturnType<typeof setTimeout> | undefined} */
      let limit;
      const observer = new MutationObserver(() => {
        clearTimeout(timer);
        timer = setTimeout(done, quiet);
      });
      function done() {
        observer.disconnect();
        clearTimeout(timer);
        clearTimeout(limit);
        resolve(undefined);
      }
      observer.observe(document, { subtree: true, childList: true, attributes: true, characterData: true });
      timer = setTimeout(done, quiet);
      limit = setTimeout(done, cap);
    });
  }

  /** @param {Iterator<unknown>} steps @param {{ quiet?: unknown, cap?: unknown, budget?: unknown }} request */
  return async (steps, request) => {
    const quiet = ms(request.quiet);
    const cap = Math.max(quiet, ms(request.cap));
    const budget = ms(request.budget);
    const start = performance.now();
    for (let step = steps.next(); !step.done; step = steps.next()) {
      const left = budget - (performance.now() - start);
      if (quiet > 0 && left > 0) await settle(Math.min(quiet, left), Math.min(cap, left));
    }
  };
}
