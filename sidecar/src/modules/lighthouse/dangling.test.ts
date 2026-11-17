import { describe, expect, it } from 'vitest';

import { Dangling } from './dangling.ts';

function lighthouseError(): Error {
  const error = new Error('Protocol error (Runtime.evaluate): Target closed');
  error.stack = `Error: ${error.message}\n    at checkForQuiet (file:///app/sidecar/node_modules/lighthouse/core/gather/driver/wait-for-condition.js:239:7)`;
  return error;
}

describe('Dangling (review 1)', () => {
  it("tolerates Lighthouse's own rejections only for a while after a run was cancelled under it", () => {
    let now = 1000;
    const dangling = new Dangling(() => now, 30_000);
    expect(dangling.tolerates(lighthouseError())).toBe(false);
    dangling.open();
    expect(dangling.tolerates(lighthouseError())).toBe(true);
    now += 30_001;
    expect(dangling.tolerates(lighthouseError())).toBe(false);
  });

  it('never tolerates a rejection from anywhere else', () => {
    const dangling = new Dangling(() => 0, 30_000);
    dangling.open();
    const other = new Error('boom');
    other.stack = 'Error: boom\n    at serve (file:///app/sidecar/dist/main.js:1:1)';
    expect(dangling.tolerates(other)).toBe(false);
    expect(dangling.tolerates('a string')).toBe(false);
  });

  it('reads Windows paths too', () => {
    const dangling = new Dangling(() => 0, 30_000);
    dangling.open();
    const error = new Error('Target closed');
    error.stack = 'Error: Target closed\n    at x (C:\\Tools\\rbrowser\\sidecar\\node_modules\\lighthouse\\core\\lib\\lh-error.js:1:1)';
    expect(dangling.tolerates(error)).toBe(true);
  });
});
