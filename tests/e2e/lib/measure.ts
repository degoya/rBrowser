// The measuring helpers of the end-to-end run (RB-099-02): percentiles as scripts/lib/perf-k1.mjs
// computes them and a poll with a give-up deadline (AGENTS.md: no fixed sleep stands in for a synchronisation, and no test holds
// the wall clock to an upper bound - the deadline only ends a wait that will never succeed).

/** The nearest-rank percentile `p` (0 to 100) of the sorted numbers; 0 for none. */
export function percentile(sorted: readonly number[], p: number): number {
  if (sorted.length === 0) return 0;
  const rank = Math.ceil((p / 100) * sorted.length);
  return sorted[Math.min(Math.max(rank, 1), sorted.length) - 1] ?? 0;
}

export interface Stats {
  count: number;
  p50Ms: number;
  p95Ms: number;
  maxMs: number;
}

const tenth = (value: number): number => Math.round(value * 10) / 10;

/** Count, p50, p95 and the largest of `values` (milliseconds), rounded to a tenth. */
export function stats(values: readonly number[]): Stats {
  const sorted = [...values].sort((a, b) => a - b);
  return {
    count: sorted.length,
    p50Ms: tenth(percentile(sorted, 50)),
    p95Ms: tenth(percentile(sorted, 95)),
    maxMs: tenth(sorted[sorted.length - 1] ?? 0),
  };
}

export interface WaitOptions {
  /** When to give up: the wait failed, not the measurement. */
  timeoutMs: number;
  /** Between two checks. */
  stepMs: number;
  /** What is waited for, for the error. */
  what: string;
}

/** Calls `check` until it answers a value other than `undefined`; fails after `timeoutMs`. */
export async function waitFor<T>(
  check: () => Promise<T | undefined>,
  options: WaitOptions,
): Promise<T> {
  const deadline = Date.now() + options.timeoutMs;
  for (;;) {
    const value = await check();
    if (value !== undefined) return value;
    if (Date.now() >= deadline) {
      throw new Error(`${options.what}: not within ${options.timeoutMs} ms`);
    }
    await new Promise((resolve) => setTimeout(resolve, options.stepMs));
  }
}

/** Resolves after `ms` - a step of a scenario (a pause between clicks), never a synchronisation. */
export function pause(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
