import { describe, expect, it } from 'vitest';
import { Agent } from '../agent';
import { isPageLocale } from './check';

describe('the locale command', () => {
  it('takes a locale with any of its fields', () => {
    expect(isPageLocale({})).toBe(true);
    expect(isPageLocale({ dir: 'rtl', lang: 'ar' })).toBe(true);
    expect(isPageLocale({ pseudo: { factor: 1.4, accents: true, rtl: false } })).toBe(true);
  });

  it('refuses what is no locale', () => {
    for (const value of [
      null,
      'rtl',
      { dir: 'up' },
      { lang: 3 },
      { pseudo: { factor: '2', accents: true, rtl: false } },
      { pseudo: { factor: 1.4, accents: 'yes', rtl: false } },
      { pseudo: { factor: Number.NaN, accents: true, rtl: false } },
    ]) {
      expect(isPageLocale(value), JSON.stringify(value)).toBe(false);
    }
  });

  it('is run by the agent: done, or refused', () => {
    document.documentElement.removeAttribute('dir');
    const agent = new Agent({ win: window, send: () => {} });
    expect(agent.command({ cmd: 'locale', locale: { dir: 'rtl' } })).toEqual({ status: 'done' });
    expect(document.documentElement.dir).toBe('rtl');
    expect(agent.command({ cmd: 'locale', locale: { dir: 'sideways' } })).toEqual({
      status: 'error',
      message: 'invalid locale',
    });
    expect(agent.command({ cmd: 'locale', locale: {} })).toEqual({ status: 'done' });
    expect(document.documentElement.hasAttribute('dir')).toBe(false);
  });
});
