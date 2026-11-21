import { describe, expect, it } from 'vitest';
import { childPath, rows } from './tree';

const DATA = { user: { name: 'Ada', langs: ['en', 'fr'] }, ok: true, 'odd key': null };

describe('the answer tree', () => {
  it('shows the top open and a container only once opened', () => {
    const closed = rows(DATA, new Set());
    expect(closed.rows.map((row) => row.path)).toEqual(['$', '$.user', '$.ok', "$['odd key']"]);
    expect(closed.rows[1]).toMatchObject({ kind: 'object', size: 2, expandable: true, open: false });
    const opened = rows(DATA, new Set(['$.user']));
    expect(opened.rows.map((row) => row.path)).toContain('$.user.langs');
    expect(opened.rows.find((row) => row.path === '$.user.name')?.text).toBe('"Ada"');
  });

  it('searches keys and values and opens the way to them', () => {
    const found = rows(DATA, new Set(), 'fr');
    expect(found.rows.map((row) => row.path)).toEqual(['$', '$.user', '$.user.langs', '$.user.langs[1]']);
    expect(found.rows.at(-1)?.match).toBe(true);
    // A key that matches shows its container closed: the values below it hold no match.
    expect(rows(DATA, new Set(), 'langs').rows.map((row) => row.path)).toEqual(['$', '$.user', '$.user.langs']);
  });

  it('stops at the limit and says so', () => {
    const many = Array.from({ length: 50 }, (_, index) => index);
    const cut = rows(many, new Set(), '', 10);
    expect(cut.rows).toHaveLength(10);
    expect(cut.cut).toBe(true);
  });

  it('names a path as JSONPath writes it', () => {
    expect(childPath('$', 'a', false)).toBe('$.a');
    expect(childPath('$', "it's", false)).toBe("$['it\\'s']");
    expect(childPath('$.a', '3', true)).toBe('$.a[3]');
  });
});
