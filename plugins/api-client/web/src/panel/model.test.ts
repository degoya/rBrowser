import { describe, expect, it } from 'vitest';
import { bodyView, byFolder, bytes, emptyRequest, millis, parsedBody, statusTone, withAuthType, withBodyKind } from './model';

describe('the API client model', () => {
  it('keeps the typed text when the body kind changes', () => {
    const json = withBodyKind({ kind: 'raw', text: '{"a":1}' }, 'json');
    expect(json).toEqual({ kind: 'json', text: '{"a":1}' });
    expect(withBodyKind(json, 'graphql')).toEqual({ kind: 'graphql', query: '{"a":1}', variables: '' });
    expect(withBodyKind(json, 'form')).toEqual({ kind: 'form', fields: [] });
    expect(withBodyKind(json, 'json')).toBe(json);
  });

  it('keeps the user name when basic becomes digest', () => {
    const basic = withAuthType({ type: 'none' }, 'basic');
    expect(basic).toEqual({ type: 'basic', username: '', password: '' });
    expect(withAuthType({ type: 'basic', username: 'ada', password: 'x' }, 'digest')).toEqual({
      type: 'digest',
      username: 'ada',
      password: 'x',
    });
    expect(withAuthType(basic, 'apiKey')).toMatchObject({ type: 'apiKey', in: 'header' });
  });

  it('groups the collections by folder, the top first, and filters them', () => {
    const requests = [
      { id: 'users/list.http', name: 'List users', method: 'GET', url: '{{baseUrl}}/users' },
      { id: 'health.http', name: 'Health', method: 'GET', url: '{{baseUrl}}/health' },
      { id: 'auth/login.http', name: 'Login', method: 'POST', url: '{{baseUrl}}/login' },
    ];
    expect(byFolder(requests).map((folder) => folder.folder)).toEqual(['', 'auth', 'users']);
    expect(byFolder(requests, 'post')).toEqual([{ folder: 'auth', requests: [requests[2]] }]);
  });

  it('shows a body by its type', () => {
    expect(bodyView({ contentType: 'application/problem+json', bodyEncoding: 'text' })).toBe('json');
    expect(bodyView({ contentType: 'text/html; charset=utf-8', bodyEncoding: 'text' })).toBe('html');
    expect(bodyView({ contentType: 'application/xml', bodyEncoding: 'text' })).toBe('xml');
    expect(bodyView({ contentType: 'image/png', bodyEncoding: 'base64' })).toBe('image');
    expect(bodyView({ contentType: 'application/octet-stream', bodyEncoding: 'base64' })).toBe('binary');
    expect(bodyView({ contentType: null, bodyEncoding: 'text' })).toBe('text');
    expect(parsedBody({ body: '{"a":[1]}', bodyEncoding: 'text' })).toEqual({ a: [1] });
    expect(parsedBody({ body: '{broken', bodyEncoding: 'text' })).toBeUndefined();
  });

  it('formats sizes, times and the status tone', () => {
    expect(bytes(512)).toBe('512 B');
    expect(bytes(1536)).toBe('1.5 KiB');
    expect(millis(3.14159)).toBe('3.1 ms');
    expect(millis(123.6)).toBe('124 ms');
    expect(millis(null)).toBe('-');
    expect(statusTone(204)).toBe('success');
    expect(statusTone(302)).toBe('info');
    expect(statusTone(404)).toBe('warning');
    expect(statusTone(503)).toBe('error');
    expect(emptyRequest().url).toBe('{{baseUrl}}/');
  });
});
