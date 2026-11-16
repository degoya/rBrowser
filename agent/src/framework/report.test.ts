import { describe, expect, it } from 'vitest';
import { MAX_VERSION, isFrameworkInfo, isFrameworkReport } from './report';

describe('isFrameworkReport', () => {
  it('accepts an empty report and complete entries', () => {
    expect(isFrameworkReport({})).toBe(true);
    expect(isFrameworkReport({ vue: { hook: true, devtools: true, version: '3.5.0' } })).toBe(true);
    expect(isFrameworkReport({ react: { hook: false, devtools: false } })).toBe(true);
    expect(
      isFrameworkReport({ vue: { hook: true, devtools: true }, react: { hook: true, devtools: false } }),
    ).toBe(true);
  });

  it('refuses anything else', () => {
    expect(isFrameworkReport(null)).toBe(false);
    expect(isFrameworkReport('vue')).toBe(false);
    expect(isFrameworkReport([])).toBe(false);
    expect(isFrameworkReport({ vue: { hook: 'yes', devtools: true } })).toBe(false);
    expect(isFrameworkReport({ react: { hook: true } })).toBe(false);
    expect(isFrameworkReport({ vue: { hook: true, devtools: true, version: 5 } })).toBe(false);
    expect(isFrameworkReport({ svelte: { hook: true, devtools: true } })).toBe(false);
    expect(isFrameworkReport({ vue: null })).toBe(false);
  });

  it('bounds the version string', () => {
    expect(MAX_VERSION).toBe(64);
    expect(isFrameworkInfo({ hook: true, devtools: true, version: 'x'.repeat(64) })).toBe(true);
    expect(isFrameworkInfo({ hook: true, devtools: true, version: 'x'.repeat(65) })).toBe(false);
    expect(isFrameworkReport({ vue: { hook: true, devtools: true, version: 'x'.repeat(65) } })).toBe(false);
  });
});
