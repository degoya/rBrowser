import { describe, expect, it } from 'vitest';
import { nameOf, statusOf } from './status';

describe('statusOf', () => {
  it('needs a view, then a report naming the framework', () => {
    expect(statusOf('vue', null, null)).toBe('noView');
    expect(statusOf('vue', 1, null)).toBe('waiting');
    expect(statusOf('vue', 1, { react: { hook: true, devtools: true } })).toBe('waiting');
  });

  it('asks for a reload while the hook is missing', () => {
    expect(statusOf('react', 1, { react: { hook: false, devtools: false } })).toBe('noHook');
    expect(statusOf('vue', 1, { vue: { hook: false, devtools: true } })).toBe('noHook');
  });

  it('tells a late hook from a Vue production build', () => {
    expect(statusOf('react', 1, { react: { hook: true, devtools: false } })).toBe('late');
    expect(statusOf('vue', 1, { vue: { hook: true, devtools: false } })).toBe('vueProd');
  });

  it('is connected once the framework registered with the hook', () => {
    expect(statusOf('vue', 2, { vue: { version: '3.5.22', hook: true, devtools: true } })).toBe('connected');
    expect(nameOf('vue')).toBe('Vue');
    expect(nameOf('react')).toBe('React');
  });
});
