import { describe, expect, it } from 'vitest';
import { createRecent, RECENT_STORAGE_KEY } from './recent';

function memoryStorage(): Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> & { data: Map<string, string> } {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (key) => data.get(key) ?? null,
    setItem: (key, value) => void data.set(key, value),
    removeItem: (key) => void data.delete(key),
  };
}

describe('the recent list', () => {
  it('keeps the newest first, once each, up to the limit', () => {
    const storage = memoryStorage();
    const recent = createRecent(storage);
    recent.push('a', 3);
    recent.push('b', 3);
    recent.push('a', 3);
    recent.push('c', 3);
    recent.push('d', 3);
    expect(recent.list()).toEqual(['d', 'c', 'a']);
    expect(JSON.parse(storage.data.get(RECENT_STORAGE_KEY) ?? '[]')).toEqual(['d', 'c', 'a']);
  });

  it('reads what an earlier page stored and drops what is malformed', () => {
    const storage = memoryStorage();
    storage.data.set(RECENT_STORAGE_KEY, JSON.stringify(['x', 3, 'y']));
    expect(createRecent(storage).list()).toEqual(['x', 'y']);
    storage.data.set(RECENT_STORAGE_KEY, '{not json');
    expect(createRecent(storage).list()).toEqual([]);
  });

  it('stores nothing with a limit of 0', () => {
    const storage = memoryStorage();
    const recent = createRecent(storage);
    recent.push('a', 0);
    expect(recent.list()).toEqual([]);
    expect(storage.data.has(RECENT_STORAGE_KEY)).toBe(false);
  });

  it('works in memory when the storage throws', () => {
    const throwing = {
      getItem: () => {
        throw new Error('blocked');
      },
      setItem: () => {
        throw new Error('blocked');
      },
      removeItem: () => {
        throw new Error('blocked');
      },
    };
    const recent = createRecent(throwing);
    recent.push('a', 5);
    expect(recent.list()).toEqual(['a']);
  });
});
