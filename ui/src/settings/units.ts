// The unit of a number setting and whether it takes plus and minus (RB-080-88, design.md 5.1,
// Number fields): the unit stands beside the field (`FieldControl`), read from the key - the
// suffix the schema's names already carry (`hoverThrottleMs`, `intervalSec`), and the few keys
// whose unit is only in their description (`layout.gap`, `capture.defaultDelay`). A key that
// matches no rule has no unit. Plus and minus belong to a count one clicks: a small whole number
// without a unit, never a measure, a fraction, an open range or a port.

import type { FieldKind } from './kinds';

export type Unit = 'ms' | 's' | 'px' | 'percent' | 'mib' | 'mb' | 'bytes' | 'fps' | 'm';

/** The catalogue key that names each unit. */
export const UNIT_KEYS: Readonly<Record<Unit, string>> = {
  ms: 'settings.unit.ms',
  s: 'settings.unit.s',
  px: 'settings.unit.px',
  percent: 'settings.unit.percent',
  mib: 'settings.unit.mib',
  mb: 'settings.unit.mb',
  bytes: 'settings.unit.bytes',
  fps: 'settings.unit.fps',
  m: 'settings.unit.m',
};

/** First match wins; the key is the full dotted name (`breakpoints.width` for a list's field). */
const RULES: readonly [RegExp, Unit][] = [
  [/Ms$/, 'ms'],
  [/Sec$/, 's'],
  [/Px$/, 'px'],
  [/Percent$/, 'percent'],
  [/Mib$/, 'mib'],
  [/Mb$/, 'mb'],
  [/Bytes$/, 'bytes'],
  [/(Fps|\.fps)$/, 'fps'],
  [/^capture\.defaultDelay$/, 's'],
  [/(^|\.)(gap|width|height|dockHeight|dockWidth|elementPadding)$/, 'px'],
  [/\.safeAreas\.(top|right|bottom|left)$/, 'px'],
  [/^panels\.dockSize\.(top|right|bottom|left)$/, 'px'],
  [/^vitals\.thresholds\.(lcp|fcp|inp|ttfb)\.(good|poor)$/, 'ms'],
  [/\.accuracy$/, 'm'],
];

/** The unit of the number at `key`, or `null` when it has none. */
export function unitOf(key: string): Unit | null {
  return RULES.find(([pattern]) => pattern.test(key))?.[1] ?? null;
}

/** The widest range a count still steps through by clicks. */
const CLICK_RANGE = 1000;

/** Whether the number at `key` is a count one clicks, which takes plus and minus. */
export function isClickCount(kind: FieldKind, key: string, bounds: { min?: number; max?: number }): boolean {
  if (kind !== 'integer' || unitOf(key) !== null || bounds.max === undefined) {
    return false;
  }
  return bounds.max - (bounds.min ?? 0) <= CLICK_RANGE;
}
