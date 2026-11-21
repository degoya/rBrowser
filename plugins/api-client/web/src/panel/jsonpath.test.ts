import { describe, expect, it } from 'vitest';
import { select, steps } from './jsonpath';

const DATA = {
  users: [
    { id: 1, name: 'Ada', tags: ['a'] },
    { id: 2, name: 'Grace', 'full name': 'Grace Hopper' },
  ],
  meta: { total: 2 },
};

describe('the JSONPath filter', () => {
  it('selects children, indices and wildcards', () => {
    expect(select(DATA, '$')).toEqual([DATA]);
    expect(select(DATA, '$.meta.total')).toEqual([2]);
    expect(select(DATA, '$.users[1].name')).toEqual(['Grace']);
    expect(select(DATA, '$.users[-1].id')).toEqual([2]);
    expect(select(DATA, '$.users[*].id')).toEqual([1, 2]);
    expect(select(DATA, "$.users[1]['full name']")).toEqual(['Grace Hopper']);
    expect(select(DATA, '$.meta.*')).toEqual([2]);
  });

  it('finds a name at every depth', () => {
    expect(select(DATA, '$..name')).toEqual(['Ada', 'Grace']);
    expect(select(DATA, '$..total')).toEqual([2]);
  });

  it('selects nothing that is not there and refuses what is no path', () => {
    expect(select(DATA, '$.nothing')).toEqual([]);
    expect(select(DATA, '$.users[9]')).toEqual([]);
    expect(select(DATA, 'users')).toBeNull();
    expect(select(DATA, '$.users[?(@.id)]')).toBeNull();
    expect(steps('$[')).toBeNull();
  });
});
