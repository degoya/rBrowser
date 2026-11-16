// RB-080-88: a number is typed in a `UInputNumber` with its unit beside it (design.md 5.1, Number
// fields); the unit is read from the key, and plus and minus come only with a count one clicks.
import { describe, expect, it } from 'vitest';
import { shellMessages } from '../i18n';
import type { JsonValue } from '../ipc/protocol';
import { isClickCount, UNIT_KEYS, unitOf } from './units';

describe('the unit of a number field', () => {
  it.each([
    ['sync.hoverThrottleMs', 'ms'],
    ['reload.intervalSec', 's'],
    ['analyzer.mergeTolerancePx', 'px'],
    ['envCompare.maxDiffPercent', 'percent'],
    ['sitemap.maxSitemapMib', 'mib'],
    ['logs.maxFileSizeMb', 'mb'],
    ['bus.maxPayloadBytes', 'bytes'],
    ['performance.inactiveFps', 'fps'],
    ['capture.video.fps', 'fps'],
    ['capture.defaultDelay', 's'],
    ['capture.elementPadding', 'px'],
    ['devtools.dockHeight', 'px'],
    ['panels.dockSize.bottom', 'px'],
    ['layout.gap', 'px'],
    ['breakpoints.width', 'px'],
    ['devices.items.viewport.height', 'px'],
    ['emulation.safeAreas.top', 'px'],
    ['vitals.thresholds.lcp.good', 'ms'],
    ['emulation.geolocation.accuracy', 'm'],
  ])('reads %s as %s', (key, unit) => {
    expect(unitOf(key)).toBe(unit);
  });

  it.each(['logs.maxFiles', 'cdp.port', 'vitals.thresholds.cls.good', 'emulation.dpr', 'layout.columns'])(
    'leaves %s without a unit',
    (key) => {
      expect(unitOf(key)).toBeNull();
    },
  );

  it('names every unit from the catalogue', () => {
    const catalogue = shellMessages() as unknown as { settings: { unit: Record<string, JsonValue> } };
    for (const key of Object.values(UNIT_KEYS)) {
      const leaf = key.split('.').at(-1) ?? '';
      expect(catalogue.settings.unit[leaf], key).toBeTypeOf('string');
    }
  });
});

describe('plus and minus on a number field', () => {
  it('come with a small whole count', () => {
    expect(isClickCount('integer', 'logs.maxFiles', { min: 1, max: 100 })).toBe(true);
    expect(isClickCount('integer', 'sitemap.retries', { min: 0, max: 10 })).toBe(true);
  });

  it('stay away from a measure, a fraction, an open range and a port', () => {
    expect(isClickCount('integer', 'sync.hoverThrottleMs', { min: 0, max: 1000 })).toBe(false);
    expect(isClickCount('number', 'emulation.dpr', { min: 1, max: 4 })).toBe(false);
    expect(isClickCount('integer', 'checks.keepRuns', { min: 1 })).toBe(false);
    expect(isClickCount('integer', 'cdp.port', { min: 1024, max: 65535 })).toBe(false);
  });
});
