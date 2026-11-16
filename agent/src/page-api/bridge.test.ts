import { describe, expect, it } from 'vitest';
import {
  MAX_PAYLOAD_LIMIT,
  MAX_RATE_LIMIT,
  RateLimiter,
  encodePayload,
  isBusSettings,
  isBusType,
  isPageState,
  utf8Length,
} from './bridge';
import { STATE } from './test-state';

describe('bus types', () => {
  it('takes short names of letters, digits and _ . : / -', () => {
    for (const type of ['canvas:pan', 'a', 'app/state.v2', 'x_y-z', 'A'.repeat(64)]) {
      expect(isBusType(type)).toBe(true);
    }
  });

  it('refuses empty, long, odd and non-string names', () => {
    for (const type of ['', ':pan', 'A'.repeat(65), 'a b', 'a\nb', '<x>', 7, null, {}]) {
      expect(isBusType(type)).toBe(false);
    }
  });
});

describe('payloads', () => {
  it('counts UTF-8 bytes', () => {
    expect(utf8Length('abc')).toBe(3);
    expect(utf8Length('\u00e4')).toBe(2);
    expect(utf8Length('€')).toBe(3);
    expect(utf8Length('\u{1f600}')).toBe(4);
  });

  it('encodes JSON values and undefined as null', () => {
    expect(encodePayload({ x: 1 }, 100)).toBe('{"x":1}');
    expect(encodePayload(undefined, 100)).toBe('null');
  });

  it('refuses what is not JSON with a TypeError', () => {
    const cyclic: Record<string, unknown> = {};
    cyclic.self = cyclic;
    expect(() => encodePayload(cyclic, 100)).toThrow(TypeError);
    expect(() => encodePayload(() => 1, 100)).toThrow(TypeError);
    expect(() => encodePayload(1n, 100)).toThrow(TypeError);
  });

  it('refuses a payload over the limit with a RangeError', () => {
    expect(() => encodePayload('x'.repeat(65536), 65536)).toThrow(RangeError);
    expect(encodePayload('x'.repeat(65534), 65536)).toHaveLength(65536);
  });
});

describe('settings and state', () => {
  it('holds the limits to their ranges', () => {
    expect(isBusSettings(STATE.bus)).toBe(true);
    expect(isBusSettings({ ...STATE.bus, maxPayloadBytes: MAX_PAYLOAD_LIMIT + 1 })).toBe(false);
    expect(isBusSettings({ ...STATE.bus, maxPerSecond: MAX_RATE_LIMIT + 1 })).toBe(false);
    expect(isBusSettings({ ...STATE.bus, maxPerSecond: 0 })).toBe(false);
    expect(isBusSettings({ ...STATE.bus, enabled: 'yes' })).toBe(false);
  });

  it('takes a complete page state only', () => {
    expect(isPageState(STATE)).toBe(true);
    expect(isPageState({ ...STATE, info: { ...STATE.info, label: 3 } })).toBe(false);
    expect(isPageState({ ...STATE, info: { ...STATE.info, engine: 'edge' } })).toBe(false);
    expect(isPageState({ info: STATE.info })).toBe(false);
    expect(isPageState(null)).toBe(false);
  });
});

describe('rate limiter', () => {
  it('lets the rate through per second and drops the rest', () => {
    let now = 1000;
    const limiter = new RateLimiter(100, () => now);
    let passed = 0;
    for (let i = 0; i < 1000; i += 1) if (limiter.take()) passed += 1;
    expect(passed).toBe(100);
    now += 500;
    passed = 0;
    for (let i = 0; i < 1000; i += 1) if (limiter.take()) passed += 1;
    expect(passed).toBe(50);
  });

  it('never stores more than one second of messages', () => {
    let now = 0;
    const limiter = new RateLimiter(10, () => now);
    now = 60_000;
    let passed = 0;
    for (let i = 0; i < 100; i += 1) if (limiter.take()) passed += 1;
    expect(passed).toBe(10);
  });

  it('follows a new rate', () => {
    const now = 0;
    const limiter = new RateLimiter(10, () => now);
    limiter.setRate(2);
    expect([limiter.take(), limiter.take(), limiter.take()]).toEqual([true, true, false]);
  });
});
