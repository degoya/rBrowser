import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { CLOSE_GRACE_MS, END_BUDGET_MS, EXIT_AFTER_MS, TASKKILL_TIMEOUT_MS } from './budget.ts';

/** rb-sidecar's `STOP_GRACE`: how long a stopped sidecar may take before it is killed. */
function stopGraceMs(): number {
  const source = readFileSync(new URL('../../../../crates/rb-sidecar/src/supervisor.rs', import.meta.url), 'utf8');
  const match = /pub const STOP_GRACE: Duration = Duration::from_secs\((\d+)\);/.exec(source);
  if (match?.[1] === undefined) {
    throw new Error('STOP_GRACE not found in supervisor.rs');
  }
  return Number(match[1]) * 1000;
}

describe('the budgets of the sidecar end (review 5)', () => {
  it("end a browser before the sidecar's own exit timer, and that before rb-sidecar kills it", () => {
    expect(CLOSE_GRACE_MS + TASKKILL_TIMEOUT_MS).toBeLessThan(END_BUDGET_MS + 1);
    expect(END_BUDGET_MS).toBeLessThan(EXIT_AFTER_MS);
    expect(EXIT_AFTER_MS).toBeLessThan(stopGraceMs());
  });
});
