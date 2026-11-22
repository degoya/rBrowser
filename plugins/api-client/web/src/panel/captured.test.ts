// A request the network panel opened in the API client (RB-096-08): the draft in the panel's
// state, read without trusting its shape, and the note it shows.
import { describe, expect, it } from 'vitest';
import { draftNote, draftOf, editedSince, snapshotOf } from './captured';
import type { ApiRequest } from './model';

const REQUEST = { name: 'GET /v1/me', method: 'GET', url: 'https://a.test/v1/me', headers: [], body: { kind: 'none' }, auth: { type: 'none' } };
const t = (key: string, params: Record<string, unknown> = {}): string => `${key}${JSON.stringify(params)}`;

describe('a captured draft', () => {
  it('is read from the state only in its shape', () => {
    expect(draftOf({ draft: { request: REQUEST, folder: 'a-test', variables: [] } })?.folder).toBe('a-test');
    expect(draftOf({})).toBeNull();
    expect(draftOf({ draft: { folder: 'a-test' } })).toBeNull();
    expect(draftOf({ draft: { request: { name: 'x' }, folder: 'a-test', variables: [] } })).toBeNull();
    expect(draftOf({ draft: { request: REQUEST, folder: 3, variables: [] } })).toBeNull();
    expect(draftOf({ draft: { request: REQUEST, folder: 'a-test' } })?.variables).toEqual([]);
  });

  it('says it is not saved yet, and what still needs a value, as a warning then', () => {
    const plain = draftOf({ draft: { request: REQUEST, folder: 'a-test', variables: [{ name: 'a-test-token', value: 'kept' }] } });
    expect(plain && draftNote(plain, t)).toEqual({ tone: 'info', text: 'panel.capture.opened{"name":"GET /v1/me"}' });
    const open = draftOf({
      draft: { request: REQUEST, folder: 'a-test', variables: [{ name: 'a-test-cookie', value: 'missing' }], bodyMissing: 'tooLong', incomplete: true },
    });
    expect(open && draftNote(open, t)).toEqual({
      tone: 'warn',
      text: 'panel.capture.opened{"name":"GET /v1/me"} panel.capture.missing{"names":"a-test-cookie"} panel.capture.body.tooLong{} panel.capture.incomplete{}',
      setting: { key: 'api-client.secrets', label: 'panel.openVariables{}' },
    });
  });

  it('says a body it does not hold and values that were not kept', () => {
    const unread = draftOf({ draft: { request: REQUEST, folder: 'a-test', variables: [], bodyMissing: 'unread', warning: 'secrets_not_saved' } });
    expect(unread && draftNote(unread, t)).toEqual({
      tone: 'warn',
      text: 'panel.capture.opened{"name":"GET /v1/me"} panel.capture.body.unread{} panel.capture.secretsNotKept{}',
    });
    const unchecked = draftOf({ draft: { request: REQUEST, folder: 'a-test', variables: [], bodyMissing: 'unchecked' } });
    expect(unchecked && draftNote(unchecked, t).text).toContain('panel.capture.body.unchecked{}');
    const odd = draftOf({ draft: { request: REQUEST, folder: 'a-test', variables: [], bodyMissing: 'other' } });
    expect(odd && draftNote(odd, t)).toEqual({ tone: 'info', text: 'panel.capture.opened{"name":"GET /v1/me"}' });
  });

  // The w31 security review: a value kept in place of a different one is said.
  it('names the variables whose value replaced the one kept before', () => {
    const replaced = draftOf({ draft: { request: REQUEST, folder: 'a-test', variables: [{ name: 'a-test-token', value: 'replaced' }] } });
    expect(replaced && draftNote(replaced, t)).toEqual({
      tone: 'warn',
      text: 'panel.capture.opened{"name":"GET /v1/me"} panel.capture.replaced{"names":"a-test-token"}',
    });
  });
});

describe('unsaved edits', () => {
  it('are the editor content changed since it was loaded, saved or emptied', () => {
    const loaded = { ...REQUEST, options: { timeoutMs: 30000, followRedirects: true, verifyTls: true } } as unknown as ApiRequest;
    const baseline = snapshotOf(loaded);
    expect(editedSince(structuredClone(loaded), baseline)).toBe(false);
    expect(editedSince({ ...loaded, url: 'https://a.test/v1/you' }, baseline)).toBe(true);
    expect(editedSince({ ...loaded, headers: [{ name: 'X', value: '1', enabled: true }] }, baseline)).toBe(true);
  });
});
