// The network panel's model (RB-070-02, on Nuxt UI since RB-080-84).
import { describe, expect, it } from 'vitest';
import * as M from './panel/model';
import type { HeldEntry } from './panel/types';

describe('the network model', () => {
  it('names a row by method and URL, and its status by its views', () => {
    const seen = { id: '1', fromCache: false, time: 1, count: 1 };
    const row = { method: 'GET', url: 'http://x.test/a', type: 'Fetch', partial: false, differs: false, views: { '1': { ...seen, status: 200 }, '2': { ...seen, status: 304 } } };
    expect(M.keyOf(row)).toBe('GET http://x.test/a');
    expect(M.statusOf(row)).toBe('200 / 304');
    expect(M.statusOf({ ...row, views: { '1': { ...seen, failed: 'net::ERR_BLOCKED_BY_CLIENT' } } })).toBe('blocked');
    expect(M.statusOf({ ...row, views: { '1': { ...seen, failed: 'net::ERR_FAILED' } } })).toBe('failed');
    expect(M.statusOf({ ...row, views: {} })).toBe('-');
    expect(M.viewColor(9)).toBe(M.PALETTE[1]);
  });

  it('shows sizes, times and short URLs', () => {
    expect(M.size(512)).toBe('512 B');
    expect(M.size(2048)).toBe('2.0 kB');
    expect(M.size(null)).toBe('-');
    expect(M.duration(41.5)).toBe('42 ms');
    expect(M.duration(1500)).toBe('1.50 s');
    expect(M.shortUrl('http://127.0.0.1:8080/network/img/hero-480.png')).toBe('hero-480.png');
    expect(M.shortUrl('http://127.0.0.1:8080/')).toBe('/');
  });

  it('escapes a URL into a glob matching only itself', () => {
    expect(M.escapeGlob('http://a.test/x?a=*&b=\\')).toBe('http://a.test/x\\?a=\\*&b=\\\\');
  });

  it('makes the list arguments from the filters', () => {
    expect(M.listArgs({ search: '  api ', type: 'Image', status: 'error', differences: true, endpoint: '' })).toEqual({
      limit: 150, search: 'api', types: ['Image'], status: 'error', differences: true,
    });
    expect(M.listArgs({ search: '', type: '', status: 'all', differences: false, endpoint: '' })).toEqual({ limit: 150 });
  });

  it('makes a mock from the editor or names the wrong field', () => {
    const form = { pattern: '*/api/user', method: 'get', action: 'mock', status: '500', delay: '800', body: '{}' };
    expect(M.mockArgs(form)).toEqual({
      ok: true,
      args: { match: { urlPattern: '*/api/user', method: 'GET' }, mock: { status: 500, delayMs: 800, body: '{}' } },
    });
    expect(M.mockArgs({ ...form, action: 'block' })).toEqual({
      ok: true, args: { match: { urlPattern: '*/api/user', method: 'GET' }, block: true },
    });
    expect(M.mockArgs({ ...form, status: '99' })).toEqual({ ok: false, error: 'editor.badStatus' });
    expect(M.mockArgs({ ...form, delay: '70000' })).toEqual({ ok: false, error: 'editor.badDelay' });
    expect(M.mockArgs({ ...form, pattern: ' ' })).toEqual({ ok: false, error: 'editor.badPattern' });
    expect(M.mockArgs({ ...form, method: 'G-T' })).toEqual({ ok: false, error: 'editor.badMethod' });
  });

  it('moves the focus with the arrow keys, Home and End', () => {
    expect(M.moveFocus(0, 'ArrowDown', 3)).toBe(1);
    expect(M.moveFocus(2, 'ArrowDown', 3)).toBe(2);
    expect(M.moveFocus(1, 'Home', 3)).toBe(0);
    expect(M.moveFocus(0, 'End', 3)).toBe(2);
    expect(M.moveFocus(0, 'x', 3)).toBeNull();
  });

  it('fills the held editor once and sends back only what changed', () => {
    const held: HeldEntry = {
      id: 'h1', key: 'k', view: 1, stage: 'request', method: 'POST', url: 'http://x.test/a',
      headers: [['cookie', '***']], body: '{"a":1}', bodyTruncated: false, resourceType: 'Fetch', breakpoint: 'b', since: 1,
    };
    const fields = M.heldFields(held);
    expect(fields).toEqual({ url: 'http://x.test/a', status: '', headers: 'cookie: ***', body: '{"a":1}' });
    expect(M.heldEdits(held, fields)).toEqual({});
    expect(M.heldEdits(held, { ...fields, url: 'http://x.test/b', body: '{}' })).toEqual({ url: 'http://x.test/b', body: '{}' });
    const response: HeldEntry = { ...held, stage: 'response', response: { status: 200, headers: [['content-type', 'text/html']] } };
    expect(M.heldEdits(response, { ...M.heldFields(response), status: '500' })).toEqual({ status: 500 });
    expect(M.contentType('x-a: 1\nContent-Type: application/json')).toBe('application/json');
  });

  it('makes a breakpoint from the form and names the wrong field', () => {
    const form = { pattern: ' */api/* ', regex: true, method: 'post', stage: 'response', views: '1, 2' };
    expect(M.breakpointArgs(form)).toEqual({
      ok: true, args: { urlPattern: '*/api/*', stage: 'response', regex: true, method: 'POST', views: [1, 2] },
    });
    expect(M.breakpointArgs({ ...form, regex: false, method: '', views: '' })).toEqual({
      ok: true, args: { urlPattern: '*/api/*', stage: 'response' },
    });
    expect(M.breakpointArgs({ ...form, pattern: ' ' })).toEqual({ ok: false, error: 'breakpoints.badPattern' });
    expect(M.breakpointArgs({ ...form, views: 'a' })).toEqual({ ok: false, error: 'breakpoints.badViews' });
    expect(M.breakpointArgs({ ...form, method: 'G-T' })).toEqual({ ok: false, error: 'editor.badMethod' });
  });

  it('reads and writes header lines', () => {
    expect(M.parseHeaderLines('a: 1\n\n b :2: x\nflag')).toEqual([['a', '1'], ['b', '2: x'], ['flag', '']]);
    expect(M.headerLines([['a', '1'], ['b', '2']])).toBe('a: 1\nb: 2');
  });

  it('makes a composed request from the form', () => {
    const form = { method: '', url: 'https://a.test/x', headers: 'x-a: 1', body: '', view: '2', times: '12' };
    expect(M.composeArgs(form)).toEqual({
      ok: true, args: { request: { method: 'GET', url: 'https://a.test/x', headers: [['x-a', '1']] }, view: 2, times: 10 },
    });
    expect(M.composeArgs({ ...form, view: '', times: '1', body: 'b' })).toEqual({
      ok: true, args: { request: { method: 'GET', url: 'https://a.test/x', headers: [['x-a', '1']], body: 'b' } },
    });
    expect(M.composeArgs({ ...form, url: 'ftp://x' })).toEqual({ ok: false, error: 'compose.badUrl' });
    expect(M.composeArgs({ ...form, method: '1' })).toEqual({ ok: false, error: 'editor.badMethod' });
  });
});
