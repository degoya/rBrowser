import { describe, expect, it, vi } from 'vitest';

import { percentile, stats, waitFor } from './measure.ts';

describe('percentile', () => {
  it('takes the nearest rank, as scripts/lib/perf-k1.mjs does', () => {
    const sorted = Array.from({ length: 100 }, (_, i) => i + 1);
    expect(percentile(sorted, 95)).toBe(95);
    expect(percentile(sorted, 50)).toBe(50);
    expect(percentile([7], 95)).toBe(7);
  });

  it('is 0 for no values', () => {
    expect(percentile([], 95)).toBe(0);
  });
});

describe('stats', () => {
  it('sorts and rounds to a tenth', () => {
    expect(stats([30.04, 10, 20])).toEqual({ count: 3, p50Ms: 20, p95Ms: 30, maxMs: 30 });
  });

  it('is all zero without values', () => {
    expect(stats([])).toEqual({ count: 0, p50Ms: 0, p95Ms: 0, maxMs: 0 });
  });
});

describe('waitFor', () => {
  it('answers the first value the check gives', async () => {
    let calls = 0;
    const value = await waitFor(async () => (++calls === 3 ? 'ready' : undefined), {
      timeoutMs: 10_000,
      stepMs: 1,
      what: 'the third call',
    });
    expect(value).toBe('ready');
    expect(calls).toBe(3);
  });

  it('gives up after its deadline with what it waited for', async () => {
    vi.useFakeTimers();
    try {
      const waiting = waitFor(async () => undefined, { timeoutMs: 500, stepMs: 100, what: 'the badge' });
      const failed = expect(waiting).rejects.toThrow('the badge: not within 500 ms');
      await vi.advanceTimersByTimeAsync(600);
      await failed;
    } finally {
      vi.useRealTimers();
    }
  });
});
