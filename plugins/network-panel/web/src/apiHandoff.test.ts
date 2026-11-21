// Requests of the list taken into the API client (RB-096-08), without the DOM: which instance of
// a request goes, what Save shown sends, and how the outcome reads.
import { describe, expect, it } from 'vitest';
import { MAX_SAVED, missingOf, newestOf, openedLine, ownErrorKey, savedLine, toSave } from './panel/apiHandoff';
import type { ListRow } from './panel/types';

const row = (url: string, views: Record<string, { id: string; time?: number }>): ListRow => ({
  method: 'GET',
  url,
  type: 'Fetch',
  partial: false,
  differs: false,
  views,
});

const t = (key: string, params: Record<string, unknown> = {}): string => `${key}${JSON.stringify(params)}`;

describe('taking requests into the API client', () => {
  it('takes the newest instance any view saw', () => {
    expect(newestOf(row('http://a.test/x', { '1': { id: '1.3', time: 5 }, '2': { id: '2.9', time: 9 } }))).toEqual({ view: 2, id: '2.9' });
    expect(newestOf(row('http://a.test/x', { '1': { id: '1.3' } }))).toEqual({ view: 1, id: '1.3' });
    expect(newestOf(row('http://a.test/x', {}))).toBeNull();
    expect(newestOf(row('http://a.test/x', { '1': { id: '' } }))).toBeNull();
  });

  it('saves every shown request once, at most the limit, and says when it cut', () => {
    const rows = [row('http://a.test/1', { '1': { id: '1.1' } }), row('http://a.test/2', {}), row('http://a.test/3', { '2': { id: '2.3' } })];
    expect(toSave(rows)).toEqual({ requests: [{ view: 1, id: '1.1' }, { view: 2, id: '2.3' }], cut: false });
    const many = Array.from({ length: MAX_SAVED + 5 }, (_, n) => row(`http://a.test/${n}`, { '1': { id: `1.${n}` } }));
    const saved = toSave(many);
    expect(saved.requests).toHaveLength(MAX_SAVED);
    expect(saved.cut).toBe(true);
  });

  it('names the variables without a value', () => {
    expect(
      missingOf([
        { name: 'a-token', value: 'kept' },
        { name: 'a-cookie', value: 'missing' },
        { name: 'a-key', value: 'present' },
      ]),
    ).toEqual(['a-cookie']);
  });

  it('says what was saved, skipped and still needs a value, as a warning then', () => {
    const whole = { cutAt: 0, partial: false };
    const ok = savedLine({ saved: [{ id: 'a/get.http', name: 'GET /' }], skipped: [], variables: [{ name: 'a-token', value: 'kept' }] }, whole, t);
    expect(ok.tone).toBe('ok');
    expect(ok.text).toBe('api.saved{"saved":1,"skipped":0}');
    const warn = savedLine(
      { saved: [], skipped: [{ name: 'GET /', reason: 'exists' }], variables: [{ name: 'a-token', value: 'missing' }] },
      { cutAt: MAX_SAVED, partial: false },
      t,
    );
    expect(warn.tone).toBe('warn');
    expect(warn.text).toBe(`api.saved{"saved":0,"skipped":1} api.missing{"count":1,"names":"a-token"} api.tooMany{"most":${MAX_SAVED}}`);
  });

  it('names the limit only when the save was cut, and says apart when the list shows only part', () => {
    const answer = { saved: [{ id: 'a/get.http', name: 'GET /' }], skipped: [], variables: [] };
    const partial = savedLine(answer, { cutAt: 0, partial: true }, t);
    expect(partial).toEqual({ tone: 'warn', text: 'api.saved{"saved":1,"skipped":0} api.partial{}' });
    expect(savedLine(answer, { cutAt: MAX_SAVED, partial: true }, t).text).toBe(
      `api.saved{"saved":1,"skipped":0} api.tooMany{"most":${MAX_SAVED}} api.partial{}`,
    );
  });

  it('says the bodies it could not take and values that were not kept', () => {
    const answer = {
      saved: [
        { id: 'a/post.http', name: 'POST /', bodyMissing: 'unread' },
        { id: 'a/get.http', name: 'GET /', bodyMissing: null },
      ],
      skipped: [],
      variables: [{ name: 'a-token', value: 'missing' as const }],
      warning: 'secrets_not_saved',
    };
    expect(savedLine(answer, { cutAt: 0, partial: false }, t)).toEqual({
      tone: 'warn',
      text: 'api.saved{"saved":2,"skipped":0} api.missing{"count":1,"names":"a-token"} api.bodyMissing{"count":1} api.errors.secrets_not_saved{}',
      setting: { key: 'api-client.secrets', label: 'api.setting_link{}' },
    });
    expect(openedLine({ request: { name: 'GET /' }, folder: 'a', variables: [] }, t)).toEqual({ tone: 'ok', text: 'api.opened{"name":"GET /"}' });
    expect(openedLine({ request: { name: 'POST /' }, folder: 'a', variables: [], bodyMissing: 'unchecked', warning: 'secrets_not_saved' }, t)).toEqual({
      tone: 'warn',
      text: 'api.opened{"name":"POST /"} api.bodyMissing{"count":1} api.errors.secrets_not_saved{}',
    });
  });

  // RB-094-21: "Set them in Settings > Network > API client" links to the private values.
  it('links a line with variables still without a value to the private values, and no other', () => {
    const missing = { request: { name: 'GET /' }, folder: 'a', variables: [{ name: 'a-token', value: 'missing' as const }] };
    expect(openedLine(missing, t).setting).toEqual({ key: 'api-client.secrets', label: 'api.setting_link{}' });
    expect(openedLine({ ...missing, variables: [] }, t).setting).toBeUndefined();
  });

  it('turns the API client failures it knows into its own sentences', () => {
    expect(ownErrorKey('plugin.api-client.errors.untrusted')).toBe('api.errors.untrusted');
    expect(ownErrorKey('plugin.api-client.errors.capture_gone')).toBe('api.errors.capture_gone');
    expect(ownErrorKey('plugin.api-client.errors.capture_template')).toBe('api.errors.capture_template');
    expect(ownErrorKey('plugin.api-client.errors.something_else')).toBeNull();
    expect(ownErrorKey('plugins.error.busy')).toBeNull();
  });

  // The w31 security review: a request holding a {{ of the page is skipped as `template`, and a
  // value kept in place of another says so.
  it('says the requests skipped for a template of the page and the values replaced', () => {
    const answer = {
      saved: [{ id: 'a/get.http', name: 'GET /' }],
      skipped: [
        { name: null, reason: 'template' },
        { name: 'GET /', reason: 'exists' },
      ],
      variables: [
        { name: 'a-token', value: 'replaced' as const },
        { name: 'a-key', value: 'kept' as const },
      ],
    };
    expect(savedLine(answer, { cutAt: 0, partial: false }, t)).toEqual({
      tone: 'warn',
      text: 'api.saved{"saved":1,"skipped":2} api.replaced{"count":1,"names":"a-token"} api.template{"count":1}',
    });
    expect(openedLine({ request: { name: 'GET /' }, folder: 'a', variables: [{ name: 'a-token', value: 'replaced' }] }, t)).toEqual({
      tone: 'warn',
      text: 'api.opened{"name":"GET /"} api.replaced{"count":1,"names":"a-token"}',
    });
  });
});
