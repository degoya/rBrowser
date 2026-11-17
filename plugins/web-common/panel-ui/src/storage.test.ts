import { describe, expect, it } from 'vitest';
import { ensureStorage, memoryStorage } from './storage';

describe('the storage stand-in', () => {
  it('keeps items in memory', () => {
    const storage = memoryStorage();
    storage.setItem('a', '1');
    expect(storage.getItem('a')).toBe('1');
    expect(storage.length).toBe(1);
    expect(storage.key(0)).toBe('a');
    storage.removeItem('a');
    expect(storage.getItem('a')).toBeNull();
  });

  it('replaces only a storage the frame cannot read', () => {
    const target = {} as Window;
    Object.defineProperty(target, 'localStorage', {
      configurable: true,
      get() {
        throw new DOMException('denied', 'SecurityError');
      },
    });
    Object.defineProperty(target, 'sessionStorage', { configurable: true, value: memoryStorage() });
    const own = target.sessionStorage;
    expect(ensureStorage(target)).toEqual(['localStorage']);
    target.localStorage.setItem('k', 'v');
    expect(target.localStorage.getItem('k')).toBe('v');
    expect(target.sessionStorage).toBe(own);
  });
});
