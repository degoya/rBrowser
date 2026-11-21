// The Endpoints tab's model (RB-096-06): the arguments of `endpoints`, the status mix, the
// compression and a finding's text key.
import { describe, expect, it } from 'vitest';
import * as E from './panel/endpointModel';
import { listArgs } from './panel/model';

describe('the endpoint model', () => {
  it('asks for the endpoints with the filters set', () => {
    expect(E.endpointArgs({ search: '', sort: 'p95', currentPage: false })).toEqual({ limit: 100 });
    expect(E.endpointArgs({ search: '  users ', sort: 'calls', currentPage: true })).toEqual({ limit: 100, search: 'users', sort: 'calls', currentPage: true });
  });

  it('says the status mix most frequent first and failures last', () => {
    expect(E.statusMix({ '200': 3, '404': 1, failed: 2 })).toBe('200 ×3 · 404 · failed ×2');
    expect(E.statusMix({})).toBe('-');
    expect(E.statusMix({ failed: 1, '200': 1 }, 'no answer')).toBe('200 · no answer');
  });

  it('names the compression of the answers', () => {
    expect(E.encodingMix({ br: 2, none: 1 })).toEqual([{ name: 'br', count: 2 }, { name: 'none', count: 1 }]);
  });

  it('turns a finding message key into the panel key below the prefix', () => {
    expect(E.messageKey('plugin.network-panel.findings.endpoint.slow')).toBe('findings.endpoint.slow');
    expect(E.messageKey('other.key')).toBe('other.key');
  });

  it('filters the request list by an endpoint', () => {
    expect(listArgs({ search: '', type: '', status: 'all', differences: false, endpoint: 'GET https://a.test/api' })).toEqual({ limit: 150, endpoint: 'GET https://a.test/api' });
  });
});
