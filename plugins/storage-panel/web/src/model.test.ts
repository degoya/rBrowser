// The storage panel's model (RB-070-01, a TypeScript module since RB-080-84): the columns by
// session, which cells differ, the grid's keyboard model and the arguments of the calls.
import { describe, expect, it } from 'vitest';
import * as M from './panel/model';
import type { StorageCell, StoragePin, StorageRow, StorageView } from './panel/model';
import { writeArgs } from './panel/write';

function view(id: number, session: string, status: StorageView['status'] = 'ok'): StorageView {
  return { id, label: `View ${id}`, breakpoint: `b${id}`, engine: 'chromium', session, url: '', origin: null, status };
}

function row(cells: Record<string, StorageCell | null>, extra: Partial<StorageRow> = {}): StorageRow {
  return { id: 'r', scope: 'http://127.0.0.1:18767', name: 'featureFlag', state: 'same', pinned: false, cells, ...extra };
}

describe('the storage model', () => {
  it('groups the columns by session in the order sessions first appear', () => {
    const { columns, groups } = M.arrange([view(1, 'default'), view(2, 'admin'), view(3, 'default')]);
    expect(columns.map((v) => v.id)).toEqual([1, 3, 2]);
    expect(groups).toEqual([
      { session: 'default', span: 2 },
      { session: 'admin', span: 1 },
    ]);
  });

  it('marks the cells outside the largest agreeing group', () => {
    const differs = M.differing(
      row({ '1': { value: 'off', length: 3 }, '2': { value: 'off', length: 3 }, '3': { value: 'on', length: 2 }, '4': null }),
    );
    expect([...differs]).toEqual([3]);
    expect(M.differing(row({ '1': { value: 'a', length: 1 }, '2': null })).size).toBe(0);
    const flags = M.differing(
      row({ '1': { value: 'a', length: 1, httpOnly: true }, '2': { value: 'a', length: 1, httpOnly: false }, '3': { value: 'a', length: 1, httpOnly: false } }),
    );
    expect([...flags]).toEqual([1]);
  });

  it('tells a value, a missing entry, another page and an unread view apart', () => {
    const r = row({ '1': { value: 'yes', length: 3 }, '3': null });
    expect(M.cellState(r, view(1, 'default'))).toBe('value');
    expect(M.cellState(r, view(3, 'admin'))).toBe('missing');
    expect(M.cellState(r, view(4, 'default'))).toBe('na');
    expect(M.cellState(r, view(5, 'default', 'unsupported'))).toBe('unsupported');
    expect(M.cellState(r, view(6, 'default', 'error'))).toBe('error');
  });

  it('moves through the grid with the arrows, Home and End', () => {
    expect(M.moveCell(0, 0, 'ArrowDown', 3, 4)).toEqual([1, 0]);
    expect(M.moveCell(2, 0, 'ArrowDown', 3, 4)).toEqual([2, 0]);
    expect(M.moveCell(1, 3, 'ArrowRight', 3, 4)).toEqual([1, 3]);
    expect(M.moveCell(1, 2, 'ArrowLeft', 3, 4)).toEqual([1, 1]);
    expect(M.moveCell(1, 2, 'Home', 3, 4)).toEqual([1, 0]);
    expect(M.moveCell(1, 2, 'End', 3, 4, true)).toEqual([2, 3]);
    expect(M.moveCell(1, 2, 'x', 3, 4)).toBeNull();
    expect(M.moveCell(0, 0, 'ArrowDown', 0, 4)).toBeNull();
  });

  it('builds the arguments of the calls', () => {
    expect(M.listArgs('local', { origin: ' 127.0.0.1 ', search: '' })).toEqual({ op: 'list', kind: 'local', origin: '127.0.0.1' });
    expect(M.entryArgs('cookies', row({}, { path: '/', scope: '127.0.0.1' }), 2)).toEqual({
      kind: 'cookies', view: 2, scope: '127.0.0.1', name: 'featureFlag', path: '/', partition: '',
    });
    expect(
      M.cookieArgs({ domain: '', path: '', expires: '2030-01-01T00:00:00Z', httpOnly: true, secure: false, sameSite: 'Lax' }),
    ).toEqual({ path: '/', httpOnly: true, secure: false, sameSite: 'Lax', expires: 1893456000 });
    expect(M.cookieArgs({ domain: '.a.example', path: '/x', expires: 'soon', httpOnly: false, secure: true, sameSite: '' })).toEqual({
      path: '/x', httpOnly: false, secure: true, domain: '.a.example',
    });
  });

  it('builds a write: the cookie attributes, or the origin of a storage entry', () => {
    const form = { name: 'consent', scope: '127.0.0.1', value: 'no', domain: '', path: '/', expires: '', httpOnly: true, secure: false, sameSite: '' };
    expect(writeArgs('cookies', 'set', 1, form)).toEqual({ op: 'set', kind: 'cookies', target: 1, name: 'consent', value: 'no', cookie: { domain: '127.0.0.1', path: '/', httpOnly: true, secure: false } });
    expect(writeArgs('local', 'delete', 'all', { ...form, scope: 'http://a.test' })).toEqual({ op: 'delete', kind: 'local', target: 'all', name: 'consent', origin: 'http://a.test' });
  });

  it('colours a value as JSON when it looks like JSON or starts like it', () => {
    expect(M.jsonOf('{"cart":[', () => null)).toBe('json');
    expect(M.jsonOf('42', () => 'json')).toBe('json');
    expect(M.jsonOf('yes', () => null)).toBeNull();
  });

  it('shows an expiry as a date or as a session cookie', () => {
    expect(M.expiresText(1893456000, 'session')).toBe('2030-01-01T00:00:00Z');
    expect(M.expiresText(null, 'session')).toBe('session');
    expect(M.expiresText(-1, 'session')).toBe('session');
  });

  it('finds the pin of a row', () => {
    const pins: StoragePin[] = [
      { id: 'a', kind: 'localStorage', origin: 'http://127.0.0.1:18767', key: 'featureFlag', value: 'on', session: null, enabled: true },
      { id: 'b', kind: 'cookies', origin: 'http://127.0.0.1:18767', key: 'consent', value: 'yes', session: null, enabled: true, cookie: { domain: null, path: '/' } },
    ];
    expect(M.pinOf(pins, 'local', row({}))?.id).toBe('a');
    expect(M.pinOf(pins, 'session', row({}))).toBeUndefined();
    expect(M.pinOf(pins, 'cookies', row({}, { name: 'consent', scope: '127.0.0.1', path: '/' }))?.id).toBe('b');
    expect(M.pinOf(pins, 'cookies', row({}, { name: 'consent', scope: '127.0.0.1', path: '/x' }))).toBeUndefined();
  });
});
