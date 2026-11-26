import { describe, expect, it } from 'vitest';

import { buildReport, exitCode, redact, renderMarkdown } from './report.ts';
import type { Entry } from './report.ts';

const passed: Entry = {
  id: 'K4',
  title: 'Leader-only submit',
  threshold: 'exactly 1 POST',
  status: 'passed',
  durationMs: 1234,
  measurements: [{ name: 'posts', value: 1, threshold: '= 1', ok: true }],
  notes: [],
  log: 'logs/K4.log',
};
const failed: Entry = {
  ...passed,
  id: 'K10',
  title: 'CDP proxy',
  status: 'failed',
  error: 'the shell is a target',
  measurements: [{ name: 'hidden targets seen', value: 1, threshold: '= 0', ok: false }],
};
const skipped: Entry = { ...passed, id: 'K11', status: 'skipped', notes: ['--allow-missing firefox'] };

const meta = {
  mode: 'wsl' as const,
  app: 'C:\\Users\\me\\AppData\\Local\\rbrowser-e2e\\rbrowser-0.9.9-windows-x64',
  version: '0.9.9',
  started: new Date('2026-11-26T10:00:00Z'),
  durationMs: 60_000,
  only: null,
};

describe('buildReport', () => {
  it('counts each status and is ok only without a failure', () => {
    const report = buildReport([passed, skipped], meta);
    expect(report.summary).toEqual({ passed: 1, failed: 0, skipped: 1, total: 2 });
    expect(report.ok).toBe(true);
    expect(report.durationMs).toBe(60_000);
    expect(report.schema).toBe(1);
    expect(buildReport([passed, failed], meta).ok).toBe(false);
  });

  it('is not ok with nothing run', () => {
    expect(buildReport([], meta).ok).toBe(false);
  });
});

describe('exitCode', () => {
  it('is 0 when every criterion passed or was allowed to skip, 1 otherwise', () => {
    expect(exitCode(buildReport([passed, skipped], meta))).toBe(0);
    expect(exitCode(buildReport([passed, failed], meta))).toBe(1);
  });
});

describe('renderMarkdown', () => {
  it('has one row per criterion with its value and threshold, and the failures below', () => {
    const text = renderMarkdown(buildReport([passed, failed, skipped], meta));
    expect(text).toContain('| K4 | Leader-only submit | passed | posts 1 (= 1) | 1.2 s |');
    expect(text).toContain('| K10 | CDP proxy | **failed** |');
    expect(text).toContain('### K10');
    expect(text).toContain('the shell is a target');
    expect(text).toContain('1 passed, 1 failed, 1 skipped');
    expect(text).toContain('Run time: 60.0 s');
  });
});

describe('redact', () => {
  it('masks bearer tokens, 64-hex secrets and token query parameters', () => {
    const token = 'ab'.repeat(32);
    expect(redact(`Authorization: Bearer ${token}`)).toBe('Authorization: Bearer ***');
    expect(redact(`"token":"${token}"`)).toBe('"token":"***"');
    expect(redact(`ws://127.0.0.1:9333/${token}/devtools`)).toBe('ws://127.0.0.1:9333/***/devtools');
    expect(redact('http://h/x?token=secret&a=1')).toBe('http://h/x?token=***&a=1');
    expect(redact('nothing secret')).toBe('nothing secret');
  });
});
