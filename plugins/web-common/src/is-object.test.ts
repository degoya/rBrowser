import { describe, expect, it } from 'vitest';

import { isObject } from './is-object';

describe('isObject', () => {
  it('takes plain objects, with or without a prototype', () => {
    expect(isObject({})).toBe(true);
    expect(isObject({ a: 1 })).toBe(true);
    expect(isObject(Object.create(null))).toBe(true);
    expect(isObject(new Map())).toBe(true);
  });

  it('refuses null, arrays and primitives', () => {
    expect(isObject(null)).toBe(false);
    expect(isObject([])).toBe(false);
    expect(isObject([{ a: 1 }])).toBe(false);
    expect(isObject(undefined)).toBe(false);
    expect(isObject('x')).toBe(false);
    expect(isObject(1)).toBe(false);
    expect(isObject(() => undefined)).toBe(false);
  });

  it('narrows to a record', () => {
    const value: unknown = { id: 'a' };
    if (isObject(value)) expect(typeof value.id).toBe('string');
    else expect.unreachable();
  });
});
