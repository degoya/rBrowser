import { describe, expect, it } from 'vitest';
import { RLO, PDF, pseudoText } from './pseudo';

const defaults = { factor: 1.4, accents: true, rtl: false };

describe('the pseudo text', () => {
  it('is at least 30 % longer at the default factor, with markers at both edges', () => {
    for (const text of ['Save', 'OK', 'Read more', 'A longer sentence of the page']) {
      const pseudo = pseudoText(text, defaults);
      expect(pseudo.length, pseudo).toBeGreaterThanOrEqual(Math.ceil(text.length * 1.3));
      expect(pseudo.startsWith('[')).toBe(true);
      expect(pseudo.endsWith(']')).toBe(true);
    }
  });

  it('grows with the factor', () => {
    const short = pseudoText('Settings', { ...defaults, factor: 1 });
    const long = pseudoText('Settings', { ...defaults, factor: 2 });
    expect(short.length).toBe('Settings'.length + 2);
    expect(long.length).toBeGreaterThanOrEqual(16 + 2);
  });

  it('swaps letters for accented ones, or keeps them', () => {
    const accented = pseudoText('Save', defaults);
    expect(accented).not.toContain('Save');
    expect(accented.normalize('NFD').replace(/[\u0300-\u036f]/g, '')).toContain('Sav');
    expect(pseudoText('Save', { ...defaults, accents: false })).toContain('Save');
    expect(pseudoText('42 %', defaults)).toContain('42 %');
  });

  it('keeps the white space around the text and white space alone', () => {
    expect(pseudoText('\n  Save  ', { ...defaults, accents: false })).toMatch(/^\n {2}\[Save~+\] {2}$/);
    expect(pseudoText('  \n\t', defaults)).toBe('  \n\t');
    expect(pseudoText('', defaults)).toBe('');
  });

  it('runs right to left inside the markers', () => {
    const pseudo = pseudoText('Save', { ...defaults, rtl: true, accents: false });
    expect(pseudo.startsWith(`[${RLO}`)).toBe(true);
    expect(pseudo.endsWith(`${PDF}]`)).toBe(true);
  });

  it('holds an absurd factor to its range', () => {
    expect(pseudoText('Save', { ...defaults, factor: 100 }).length).toBeLessThanOrEqual(4 * 3 + 2);
    expect(pseudoText('Save', { ...defaults, factor: Number.NaN }).length).toBeGreaterThan(6);
  });
});
