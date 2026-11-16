import { describe, expect, it } from 'vitest';

import { CONNECT_TIMEOUT_MS, connectOptions } from './connect.ts';

describe('connectOptions', () => {
  const endpoint = { url: 'http://127.0.0.1:9001', headers: { Authorization: 'Bearer x' } };

  it('leaves the shared default context as rBrowser set it', () => {
    // Without noDefaults Playwright sets the download behaviour browser-wide, the font families
    // and focus emulation of every view it attaches to.
    expect(connectOptions(endpoint, 1000)).toEqual({
      headers: { Authorization: 'Bearer x' },
      timeout: 1000,
      noDefaults: true,
    });
  });

  it('connects no longer than its limit', () => {
    expect(connectOptions(endpoint, 10 * CONNECT_TIMEOUT_MS).timeout).toBe(CONNECT_TIMEOUT_MS);
  });
});
