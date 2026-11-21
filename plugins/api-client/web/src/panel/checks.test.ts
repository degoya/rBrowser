import { describe, expect, it } from 'vitest';
import { CHECK_KINDS, type Report, checkLine, checkOk, counted, delta, failedRuns, newCheck } from './checks';

describe('the checks of a request', () => {
  it('are written in the lines the core reads', () => {
    expect(checkLine({ kind: 'status', expected: '2xx' })).toBe('status 2xx');
    expect(checkLine({ kind: 'json', path: '$.user.name', compare: 'equals', value: '"Ada"' })).toBe('json $.user.name == "Ada"');
    expect(checkLine({ kind: 'json', path: '$.tags', compare: 'contains', value: 'admin' })).toBe('json $.tags contains admin');
    expect(checkLine({ kind: 'header', name: 'X-Empty', compare: 'equals', value: '' })).toBe('header X-Empty ==');
    expect(checkLine({ kind: 'time', belowMs: 500 })).toBe('time < 500');
  });

  it('start from a value most requests want and are held to their form', () => {
    for (const kind of CHECK_KINDS) expect(newCheck(kind).kind).toBe(kind);
    expect(checkOk(newCheck('status'))).toBe(true);
    expect(checkOk({ kind: 'status', expected: '600' })).toBe(false);
    expect(checkOk({ kind: 'status', expected: '4xx' })).toBe(true);
    expect(checkOk({ kind: 'json', path: 'user', compare: 'equals', value: '' })).toBe(false);
    expect(checkOk({ kind: 'header', name: 'Bad Name', compare: 'equals', value: '' })).toBe(false);
    expect(checkOk({ kind: 'time', belowMs: 0 })).toBe(false);
    expect(checkOk({ kind: 'time', belowMs: 1.5 })).toBe(false);
  });
});

describe('a run and a benchmark', () => {
  it('list the requests that did not pass and count by key', () => {
    const report: Report = {
      at: 0,
      tally: { requests: 2, passed: 1, failed: 1, errors: 0, checks: 1, checksFailed: 1 },
      requests: [
        { id: 'a.http', name: 'A', method: 'GET', url: '/a', passed: true },
        { id: 'b.http', name: 'B', method: 'GET', url: '/b', passed: false },
      ],
      cut: false,
    };
    expect(failedRuns(report).map((ran) => ran.id)).toEqual(['b.http']);
    expect(counted({ '503': 1, '200': 5 })).toBe('200: 5, 503: 1');
    expect(delta(-30)).toBe('-30 ms');
    expect(delta(2.25)).toBe('+2.3 ms');
  });
});
