import { describe, expect, it } from 'vitest';
import { caseTarget, elementPathCases } from './element-path-cases';
import { elementPath } from './element-path';

describe('elementPath', () => {
  const cases = elementPathCases();

  it('reads the shared fixture, shadow cases included', () => {
    expect(cases.length).toBeGreaterThanOrEqual(10);
    expect(cases.some((entry) => entry.shadow)).toBe(true);
  });

  it.each(cases.map((entry) => [entry.name, entry] as const))('%s', (_name, entry) => {
    expect(elementPath(caseTarget(entry))).toBe(entry.path);
  });

  it('is the tag alone for an element without a parent', () => {
    expect(elementPath(document.createElement('DIV'))).toBe('div');
  });

  it('numbers the top-level children of a shadow root among themselves', () => {
    document.body.innerHTML = '<section></section>';
    const host = document.querySelector('section');
    if (!host) throw new Error('no host');
    const root = host.attachShadow({ mode: 'open' });
    root.innerHTML = '<p></p><p></p>';
    expect(elementPath(root.querySelectorAll('p')[0] as Element)).toBe('html > body > section >>> p:nth-of-type(1)');
  });
});
