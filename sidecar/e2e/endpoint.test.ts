// The endpoint resolution of the Playwright example (examples/playwright/endpoint.ts,
// RB-040-06): the environment and the answer of `cdp_endpoint`, never a written-in token.

import { describe, expect, it } from 'vitest';

import { endpointFromAnswer, resolveEndpoint } from '../../examples/playwright/endpoint.ts';

const TOKEN = 'a'.repeat(64);

const ANSWER = {
  enabled: true,
  url: 'http://127.0.0.1:9333',
  urlWithToken: `http://127.0.0.1:9333/t/${TOKEN}`,
  webSocketDebuggerUrl: 'ws://127.0.0.1:9333/devtools/browser/abc',
  headers: { Authorization: `Bearer ${TOKEN}` },
  wsl: { available: false, reason: 'no WSL' },
};

const noFile = (): string => {
  throw new Error('no file');
};

describe('resolveEndpoint', () => {
  it('sends the token of RBROWSER_CDP_TOKEN as a bearer header', () => {
    const env = { RBROWSER_CDP_URL: 'http://127.0.0.1:9333', RBROWSER_CDP_TOKEN: TOKEN };
    expect(resolveEndpoint(env, noFile)).toEqual({
      url: 'http://127.0.0.1:9333',
      headers: { Authorization: `Bearer ${TOKEN}` },
    });
  });

  it('takes a URL with the token in its path as it is', () => {
    const env = { RBROWSER_CDP_URL: ANSWER.urlWithToken };
    expect(resolveEndpoint(env, noFile)).toEqual({ url: ANSWER.urlWithToken, headers: {} });
  });

  it('reads the answer of cdp_endpoint from the file RBROWSER_CDP_ENDPOINT names', () => {
    const read = (path: string): string => {
      expect(path).toBe('/tmp/endpoint.json');
      return JSON.stringify(ANSWER);
    };
    expect(resolveEndpoint({ RBROWSER_CDP_ENDPOINT: '/tmp/endpoint.json' }, read)).toEqual({
      url: ANSWER.url,
      headers: ANSWER.headers,
    });
  });

  it('fails without an endpoint and never with the token in the message', () => {
    expect(() => resolveEndpoint({}, noFile)).toThrow(/RBROWSER_CDP_URL/);
    const broken = (): string => `{"headers": {"Authorization": "Bearer ${TOKEN}"`;
    expect(() => resolveEndpoint({ RBROWSER_CDP_ENDPOINT: '/x.json' }, broken)).toThrow(
      /no readable JSON file: \/x\.json$/,
    );
  });
});

describe('endpointFromAnswer', () => {
  it('takes the tunnel inside WSL in NAT mode', () => {
    const answer = { ...ANSWER, wsl: { url: 'http://127.0.0.1:40123', tunnel: true, networkingMode: 'nat' } };
    expect(endpointFromAnswer(answer, true).url).toBe('http://127.0.0.1:40123');
    expect(endpointFromAnswer(answer, false).url).toBe(ANSWER.url);
  });

  it('names the failed tunnel and its hint inside WSL', () => {
    const answer = { ...ANSWER, wsl: { error: 'the tunnel did not start', hint: 'rbrowser.exe cdp-tunnel' } };
    expect(() => endpointFromAnswer(answer, true)).toThrow(
      'no route from WSL: the tunnel did not start (rbrowser.exe cdp-tunnel)',
    );
  });

  it('keeps the Windows URL inside WSL when there is no WSL route', () => {
    expect(endpointFromAnswer(ANSWER, true).url).toBe(ANSWER.url);
  });

  it('refuses an answer of a proxy that is off', () => {
    expect(() => endpointFromAnswer({ enabled: false }, false)).toThrow(/CDP proxy/);
    expect(() => endpointFromAnswer('text', false)).toThrow(/no JSON object/);
  });
});
