import { describe, expect, it } from 'vitest';

import { ErrorCode, RpcError } from '../../rpc.ts';
import { CATEGORIES, MAX_WAIT_MS, readJob, readReport, readStart, readWait } from './params.ts';

const START = {
  url: 'http://localhost:5173/',
  label: 'mobile',
  settings: {
    formFactor: 'mobile',
    screenEmulation: { width: 375, height: 812, deviceScaleFactor: 3, mobile: true },
    emulatedUserAgent: 'Mozilla/5.0 (iPhone)',
    throttlingMethod: 'simulate',
    onlyCategories: ['performance', 'seo'],
  },
};

function code(run: () => unknown): number | null {
  try {
    run();
  } catch (error) {
    return error instanceof RpcError ? error.code : -1;
  }
  return null;
}

describe('readStart', () => {
  it('reads a full request', () => {
    expect(readStart(START)).toEqual({
      url: 'http://localhost:5173/',
      label: 'mobile',
      settings: {
        formFactor: 'mobile',
        screenEmulation: { width: 375, height: 812, deviceScaleFactor: 3, mobile: true, disabled: false },
        emulatedUserAgent: 'Mozilla/5.0 (iPhone)',
        throttlingMethod: 'simulate',
        onlyCategories: ['performance', 'seo'],
      },
    });
  });

  it('defaults the label, the user agent and the categories', () => {
    const request = readStart({
      url: 'https://example.com/a?b=1',
      settings: {
        formFactor: 'desktop',
        screenEmulation: { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false },
        throttlingMethod: 'devtools',
      },
    });
    expect(request.label).toBe('');
    expect(request.settings.emulatedUserAgent).toBeNull();
    expect(request.settings.onlyCategories).toEqual([...CATEGORIES]);
  });

  it('refuses every URL but http and https, and credentials in it', () => {
    for (const url of ['file:///etc/passwd', 'javascript:alert(1)', 'chrome://settings', 'about:blank', 'not a url', 'http://user:pw@host/']) {
      expect(code(() => readStart({ ...START, url }))).toBe(ErrorCode.InvalidParams);
    }
  });

  it('refuses unknown keys, so no config reaches Lighthouse beyond the listed settings', () => {
    expect(code(() => readStart({ ...START, config: { audits: ['/tmp/evil.js'] } }))).toBe(ErrorCode.InvalidParams);
    expect(code(() => readStart({ ...START, settings: { ...START.settings, plugins: ['x'] } }))).toBe(
      ErrorCode.InvalidParams,
    );
    expect(
      code(() =>
        readStart({ ...START, settings: { ...START.settings, screenEmulation: { ...START.settings.screenEmulation, x: 1 } } }),
      ),
    ).toBe(ErrorCode.InvalidParams);
  });

  it('holds the emulation and the lists to their bounds', () => {
    const bad = [
      { formFactor: 'tablet' },
      { throttlingMethod: 'fast' },
      { onlyCategories: ['pwa'] },
      { onlyCategories: [] },
      { emulatedUserAgent: 'a\nb' },
      { emulatedUserAgent: 'x'.repeat(513) },
      { screenEmulation: { width: 0, height: 800, deviceScaleFactor: 1, mobile: true } },
      { screenEmulation: { width: 375.5, height: 800, deviceScaleFactor: 1, mobile: true } },
      { screenEmulation: { width: 375, height: 100_000, deviceScaleFactor: 1, mobile: true } },
      { screenEmulation: { width: 375, height: 800, deviceScaleFactor: 0, mobile: true } },
      { screenEmulation: { width: 375, height: 800, deviceScaleFactor: 2, mobile: 'yes' } },
      { screenEmulation: { width: 375, height: 800, deviceScaleFactor: 2, mobile: false } },
    ];
    for (const change of bad) {
      expect(code(() => readStart({ ...START, settings: { ...START.settings, ...change } }))).toBe(
        ErrorCode.InvalidParams,
      );
    }
    expect(code(() => readStart({ ...START, label: 'x'.repeat(129) }))).toBe(ErrorCode.InvalidParams);
    expect(code(() => readStart(null))).toBe(ErrorCode.InvalidParams);
  });
});

describe('job parameters', () => {
  it('reads a job id and refuses anything else', () => {
    expect(readJob({ job: 'lh-1' })).toBe('lh-1');
    expect(code(() => readJob({ job: 1 }))).toBe(ErrorCode.InvalidParams);
    expect(code(() => readJob({ job: 'x'.repeat(65) }))).toBe(ErrorCode.InvalidParams);
    expect(code(() => readJob({}))).toBe(ErrorCode.InvalidParams);
  });

  it('caps the wait of a poll below the request timeout', () => {
    expect(readWait({ job: 'lh-1' })).toBe(15_000);
    expect(readWait({ job: 'lh-1', waitMs: 0 })).toBe(0);
    expect(readWait({ job: 'lh-1', waitMs: 999_999 })).toBe(MAX_WAIT_MS);
    expect(code(() => readWait({ job: 'lh-1', waitMs: -1 }))).toBe(ErrorCode.InvalidParams);
  });

  it('reads the report format', () => {
    expect(readReport({ job: 'lh-1', format: 'html' })).toEqual({ job: 'lh-1', format: 'html', offset: 0 });
    expect(readReport({ job: 'lh-1', format: 'json', offset: 512 })).toEqual({ job: 'lh-1', format: 'json', offset: 512 });
    expect(code(() => readReport({ job: 'lh-1', format: 'csv' }))).toBe(ErrorCode.InvalidParams);
    expect(code(() => readReport({ job: 'lh-1', format: 'json', offset: -1 }))).toBe(ErrorCode.InvalidParams);
  });
});
