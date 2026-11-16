// The storage operations the component runs in a view through `evaluate` (src/read.rs,
// src/write.rs, src/pins.rs): `(<this function>)(<op as JSON>)` in the plugin's isolated world,
// whose localStorage and sessionStorage are the page's (the world shares the origin). Every
// writing op names the origin it means and does nothing in a document of another one. Values are
// handed back to the component only; nothing is logged.
// @ts-check
/** @param {Record<string, any>} op */
(op) => {
  'use strict';
  const RECORD = 'rbrowser:storage-panel:pins';
  const origin = location.origin;
  const area = (/** @type {unknown} */ name) => (name === 'session' ? sessionStorage : localStorage);
  const entries = (/** @type {Storage} */ storage, /** @type {number} */ max) => {
    const out = [];
    let total = 0;
    for (let i = 0; i < storage.length; i += 1) {
      const key = storage.key(i);
      // The pins record is the plugin's own, not the page's.
      if (key === null || key === RECORD) continue;
      total += 1;
      if (out.length < max) out.push([key, storage.getItem(key) ?? '']);
    }
    return { entries: out, more: total - out.length };
  };
  try {
    if (op.op === 'read') {
      const local = op.local ? entries(localStorage, op.max) : { entries: [], more: 0 };
      const session = op.session ? entries(sessionStorage, op.max) : { entries: [], more: 0 };
      return { origin, local: local.entries, session: session.entries, more: local.more + session.more };
    }
    if (op.origin !== origin) return { ok: false, reason: 'origin', origin };
    if (op.op === 'set') {
      area(op.area).setItem(op.key, op.value);
      return { ok: true };
    }
    if (op.op === 'remove') {
      area(op.area).removeItem(op.key);
      return { ok: true };
    }
    if (op.op === 'pins') {
      // The pins of this origin: set now, and kept in this tab's sessionStorage, where the page
      // script (page/pins.js) finds them at the next document start before the page's scripts.
      for (const [key, value] of Object.entries(op.local ?? {})) localStorage.setItem(key, value);
      for (const [key, value] of Object.entries(op.session ?? {})) sessionStorage.setItem(key, value);
      const empty = Object.keys(op.local ?? {}).length + Object.keys(op.session ?? {}).length === 0;
      if (empty) sessionStorage.removeItem(RECORD);
      else sessionStorage.setItem(RECORD, JSON.stringify({ origin, local: op.local ?? {}, session: op.session ?? {} }));
      return { ok: true };
    }
    return { ok: false, reason: 'op' };
  } catch (error) {
    // Storage refused (an opaque origin, storage switched off) or full.
    const name = error && typeof error === 'object' && 'name' in error ? String(/** @type {{ name: unknown }} */ (error).name) : 'Error';
    return { ok: false, reason: name === 'QuotaExceededError' ? 'quota' : 'unavailable', origin };
  }
}
