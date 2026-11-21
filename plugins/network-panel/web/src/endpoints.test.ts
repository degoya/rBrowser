// The Endpoints tab (RB-096-06): what it asks for, how it shows an endpoint and its issues, the
// check, and the jump to the endpoint's requests in the Requests tab.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { $, answer, host, openTab, posted, settle, shownTab, start, stop } from './harness';

const P = 'plugin.network-panel.';
const KEY = 'GET https://a.test/api/users/{id}';
const STRINGS: Record<string, string> = {
  [P + 'panels.network']: 'Network',
  [P + 'tabs.endpoints']: 'Endpoints',
  [P + 'endpoints.issues']: '{count, plural, one {# issue} other {# issues}}',
  [P + 'endpoints.checked']: '{findings, plural, =0 {No findings.} one {# finding in the findings panel.} other {# findings in the findings panel.}}',
  [P + 'endpoints.filtered']: 'Endpoint {endpoint}',
  [P + 'endpoints.severity.warning']: 'Warning',
  [P + 'findings.endpoint.slow']: '{endpoint}: {p95} ms at the 95th percentile.',
};

const ENDPOINTS = {
  total: 1,
  captured: true,
  sort: 'p95',
  views: [
    { id: 1, label: 'Mobile' },
    { id: 2, label: 'Desktop' },
  ],
  endpoints: [
    {
      key: KEY, method: 'GET', origin: 'https://a.test', path: '/api/users/{id}', calls: 3, failed: 0, fromCache: 0,
      durationMs: { median: 120, p95: 3000 }, ttfbMs: { median: 60, p95: 90 },
      transferred: { total: 900, median: 300 }, decoded: { total: 3000, median: 1000 },
      statuses: { '200': 2, '404': 1 }, encodings: { br: 3 }, cacheControl: ['no-store'], validators: 0,
      views: [
        { view: 1, calls: 2, durationMs: { median: 110, p95: 120 } },
        { view: 2, calls: 1, durationMs: { median: 3000, p95: 3000 } },
      ],
      requests: [{ view: 2, id: '2.1', url: 'https://a.test/api/users/1?token=***' }],
      issues: ['endpoint.duplicate', 'endpoint.slow'],
    },
  ],
  findings: [
    { rule: 'endpoint.slow', severity: 'warning', message: P + 'findings.endpoint.slow', view: 2, path: KEY, endpoint: KEY,
      params: { endpoint: KEY, p95: '3000', median: '3000', limit: '1000' } },
  ],
};

const LIST = { rows: [], total: 0, captured: true, views: [], trusted: true, rulesRunning: true, applied: 0, mocksEnabled: true, cacheDisabled: false };

beforeEach(async () => {
  await start(STRINGS);
  await answer('list', LIST);
  await openTab('endpoints');
  await answer('endpoints', ENDPOINTS);
});

afterEach(stop);

describe('the Endpoints tab', () => {
  it('asks for the endpoints and shows each with its numbers and issues', () => {
    expect(posted.find((m) => m.tool === 'endpoints')?.args).toEqual({ limit: 100 });
    const row = $(`#endpoints [data-endpoint="${KEY}"]`).closest('tr') as HTMLElement;
    expect(row.textContent).toContain('/api/users/{id}');
    expect(row.querySelector('[data-p95]')?.textContent).toBe('3.00 s');
    expect(row.querySelector('[data-issues]')?.textContent).toBe('2 issues');
  });

  it('asks again with the sort and the current page chosen', async () => {
    $<HTMLInputElement>('#endpoint-current').click();
    await settle();
    expect([...posted].reverse().find((m) => m.tool === 'endpoints')?.args).toEqual({ limit: 100, currentPage: true });
  });

  it('shows the chosen endpoint per view with its findings in words', async () => {
    $(`#endpoints [data-endpoint="${KEY}"]`).click();
    await settle();
    expect($('#endpoint-detail')).not.toBeNull();
    expect(document.querySelectorAll('#endpoint-views tbody tr')).toHaveLength(2);
    expect($('#endpoint-views tbody').textContent).toContain('Desktop');
    expect($('#endpoint-statuses').textContent).toBe('200 ×2 · 404');
    expect($('#endpoint-findings [data-finding]').textContent).toContain(`${KEY}: 3000 ms at the 95th percentile.`);
    expect($('#endpoint-findings [data-finding]').textContent).toContain('Warning · Desktop');
  });

  it('jumps to the endpoint requests and back to every request', async () => {
    $(`#endpoints [data-endpoint="${KEY}"]`).click();
    await settle();
    $('#endpoint-requests').click();
    await settle();
    expect(shownTab('#panel-requests')).toBe(true);
    expect([...posted].reverse().find((m) => m.tool === 'list')?.args).toEqual({ limit: 150, endpoint: KEY });
    expect($('#endpoint-filter').textContent).toContain(`Endpoint ${KEY}`);
    // One read at a time: the next waits for this one's answer.
    await answer('list', LIST);
    $('#endpoint-filter-clear').click();
    await settle();
    expect($('#endpoint-filter')).toBeNull();
    expect([...posted].reverse().find((m) => m.tool === 'list')?.args).toEqual({ limit: 150 });
  });

  it('reports the findings with Check and says how many', async () => {
    $('#endpoint-check').click();
    await settle();
    expect(posted.some((m) => m.command === 'endpoints_check')).toBe(true);
    await answer('endpoints_check', { findings: 2, reported: true });
    expect($('#endpoints-note').textContent).toBe('2 findings in the findings panel.');
  });

  it('opens on the command that shows the endpoints', async () => {
    await openTab('requests');
    expect(shownTab('#panel-endpoints')).toBe(false);
    await host({ type: 'event', message: { tab: 'endpoints' } });
    expect(shownTab('#panel-endpoints')).toBe(true);
  });
});
