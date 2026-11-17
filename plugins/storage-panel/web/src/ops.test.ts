// src/ops.js as `evaluate` runs it in a view's isolated world.
import { beforeEach, describe, expect, it } from 'vitest';

import { ops } from './support';

const ORIGIN = 'http://127.0.0.1:18767';
const RECORD = 'rbrowser:storage-panel:pins';

describe('the storage operations', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
  });

  it('reads both areas, leaving out the pins record and what is past the limit', () => {
    localStorage.setItem('featureFlag', 'off');
    localStorage.setItem('theme', 'dark');
    sessionStorage.setItem('step', '2');
    sessionStorage.setItem(RECORD, '{}');
    const answer = ops({ op: 'read', local: true, session: true, max: 1 });
    expect(answer.origin).toBe(ORIGIN);
    expect((answer.local as unknown[]).length).toBe(1);
    expect(answer.session).toEqual([['step', '2']]);
    expect(answer.more).toBe(1);
    expect(ops({ op: 'read', local: false, session: false, max: 5 })).toEqual({ origin: ORIGIN, local: [], session: [], more: 0 });
  });

  it('sets and removes a key only in a document of the named origin', () => {
    expect(ops({ op: 'set', area: 'local', origin: ORIGIN, key: 'featureFlag', value: 'on' })).toEqual({ ok: true });
    expect(localStorage.getItem('featureFlag')).toBe('on');
    expect(ops({ op: 'set', area: 'session', origin: 'https://example.com', key: 'a', value: 'b' })).toEqual({
      ok: false, reason: 'origin', origin: ORIGIN,
    });
    expect(sessionStorage.getItem('a')).toBeNull();
    expect(ops({ op: 'remove', area: 'local', origin: ORIGIN, key: 'featureFlag' })).toEqual({ ok: true });
    expect(localStorage.getItem('featureFlag')).toBeNull();
    expect(ops({ op: 'drop', origin: ORIGIN })).toEqual({ ok: false, reason: 'op' });
  });

  it('applies pins and keeps their record for the next document, dropping it when empty', () => {
    ops({ op: 'pins', origin: ORIGIN, local: { featureFlag: 'on' }, session: { step: '3' } });
    expect(localStorage.getItem('featureFlag')).toBe('on');
    expect(sessionStorage.getItem('step')).toBe('3');
    expect(JSON.parse(sessionStorage.getItem(RECORD) ?? 'null')).toEqual({
      origin: ORIGIN, local: { featureFlag: 'on' }, session: { step: '3' },
    });
    ops({ op: 'pins', origin: ORIGIN, local: {}, session: {} });
    expect(sessionStorage.getItem(RECORD)).toBeNull();
  });
});
