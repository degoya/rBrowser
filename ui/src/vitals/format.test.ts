import { describe, expect, it } from 'vitest';
import { formatResource, formatVital, SHAPES } from './format';

describe('Web Vitals numbers', () => {
  it.each([
    ['lcp', 312, '312 ms'],
    ['lcp', 1840, '1.84 s'],
    ['inp', 336.4, '336 ms'],
    ['ttfb', 1000, '1.00 s'],
    ['cls', 0.1834, '0.18'],
    ['cls', 0, '0.00'],
    ['cls', 0.004, '<0.01'],
  ] as const)('%s %s reads %s', (metric, value, text) => {
    expect(formatVital(metric, value)).toBe(text);
  });

  it('gives every rating its own shape', () => {
    expect(new Set(Object.values(SHAPES)).size).toBe(3);
  });

  it('rounds the resources for the label', () => {
    expect(formatResource(12.46)).toBe('12');
    expect(formatResource(0.4)).toBe('0');
    expect(formatResource(512.6)).toBe('513');
  });
});
