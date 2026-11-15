// @ts-check
// The storage panel's page script (RB-070-01): pinned storage values set at document start,
// before the page's own scripts. It runs in the plugin's isolated world of the top frame, whose
// localStorage and sessionStorage are the page's. The pins of this origin wait in the tab's
// sessionStorage (the record src/ops.js and this script keep); they are set synchronously here,
// then the plugin is told the document started and answers the current record, which replaces
// the kept one (the plugin sets the pinned cookies itself). Nothing is logged: values are user
// data.
(() => {
  'use strict';
  if (typeof rbPlugin === 'undefined' || window !== window.top) return;
  const RECORD = 'rbrowser:storage-panel:pins';
  const plugin = rbPlugin;

  /** @param {unknown} record */
  function valid(record) {
    return !!record && typeof record === 'object' && /** @type {{ origin?: unknown }} */ (record).origin === location.origin;
  }

  /**
   * @param {Storage} storage
   * @param {unknown} values
   */
  function setAll(storage, values) {
    if (!values || typeof values !== 'object') return;
    for (const [key, value] of Object.entries(values)) {
      if (typeof value === 'string' && storage.getItem(key) !== value) storage.setItem(key, value);
    }
  }

  /**
   * The values of a record in one comparable text (the plugin and src/ops.js order keys apart).
   * @param {unknown} record
   */
  function canon(record) {
    if (!valid(record)) return '';
    const pins = /** @type {{ local?: object, session?: object }} */ (record);
    const sorted = (/** @type {object | undefined} */ values) => JSON.stringify(Object.entries(values ?? {}).sort());
    return sorted(pins.local) + sorted(pins.session);
  }

  /** @param {unknown} record */
  function apply(record) {
    if (!valid(record)) return;
    const pins = /** @type {{ local?: unknown, session?: unknown }} */ (record);
    setAll(localStorage, pins.local);
    setAll(sessionStorage, pins.session);
  }

  let kept = null;
  try {
    const text = sessionStorage.getItem(RECORD);
    kept = text ? JSON.parse(text) : null;
    apply(kept);
  } catch {
    // No storage for this document (an opaque origin, storage switched off): nothing to pin.
    return;
  }

  plugin
    .send({ type: 'loaded', origin: location.origin })
    .then((answer) => {
      const record = answer && typeof answer === 'object' ? /** @type {{ record?: unknown }} */ (answer).record : null;
      try {
        if (valid(record)) {
          sessionStorage.setItem(RECORD, JSON.stringify(record));
          // Set now only what the kept record did not set already at the start.
          if (canon(record) !== canon(kept)) apply(record);
        } else {
          sessionStorage.removeItem(RECORD);
        }
      } catch {
        // Storage full or gone: the next start tries again.
      }
    })
    .catch(() => {
      // The plugin is stopped or busy: the kept record stays for the next start.
    });

  plugin.onStop(() => {
    try {
      sessionStorage.removeItem(RECORD);
    } catch {
      // Nothing kept.
    }
  });
})();
