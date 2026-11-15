import { describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import { entrySummary, summaryOf } from './summary';

const t = createShellI18n().global.t as (key: string, args?: Record<string, unknown>) => string;

describe('the one-line summary of a list entry (RB-080-13)', () => {
  it('sums up a breakpoint: name, size, pixel ratio, engine, touch', () => {
    expect(
      summaryOf({ id: 'mobile', width: 375, height: 667, dpr: 2, engine: 'chromium', touch: true }, t),
    ).toBe('mobile · 375 × 667 · DPR 2 · Chromium · Touch');
    expect(summaryOf({ id: 'wide', label: 'Wide desktop', width: 1440 }, t)).toBe('Wide desktop · 1440 px wide');
    expect(summaryOf({ id: 'phone', device: 'iphone-15', engine: 'webkit' }, t)).toBe(
      'phone · device iphone-15 · WebKit (not Safari)',
    );
  });

  it('sums up a device preset by its viewport and kind', () => {
    expect(
      summaryOf({ id: 'pixel', kind: 'mobile', viewport: { width: 412, height: 915 }, dpr: 2.625, touch: true }, t),
    ).toBe('pixel · mobile · 412 × 915 · DPR 2.625 · Touch');
  });

  it('sums up a grid cell and a canvas item', () => {
    expect(summaryOf({ breakpoint: 'mobile', col: 1, row: 2 }, t)).toBe('mobile · column 1, row 2');
    expect(summaryOf({ breakpoint: 'desktop', col: 2, row: 1, colSpan: 2, rowSpan: 1, zoom: 0.5 }, t)).toBe(
      'desktop · column 2, row 1 · spans 2 × 1 · zoom 0.5',
    );
    expect(summaryOf({ breakpoint: 'tablet', x: 40, y: 0 }, t)).toBe('tablet · at 40, 0');
  });

  it('sums up a layout preset by its mode and its grid', () => {
    expect(
      entrySummary(
        'review',
        { mode: 'grid', layout: { columns: 3, rows: 1, cells: [{ breakpoint: 'a', col: 1, row: 1 }] } },
        t,
      ),
    ).toBe('review · grid · 3 × 1 grid · 1 cell');
    expect(entrySummary('focus', { mode: 'focus', focus: 'mobile' }, t)).toBe('focus · focus · mobile');
  });

  it('names an entry without a name, and keeps an environment readable', () => {
    expect(summaryOf({}, t)).toBe('Unnamed entry');
    expect(summaryOf({ id: 'staging', url: 'https://staging.example.test' }, t)).toBe(
      'staging · https://staging.example.test',
    );
    expect(summaryOf('plain', t)).toBe('plain');
  });
});
