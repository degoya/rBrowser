// The Nu HTML Checker engine against a stubbed `fetch`: the request it sends (POST of the source
// to the endpoint, text/html, no credentials, no redirect followed), the mapping of Nu's JSON to
// the runner's messages, and every failure of the address as an error with its reason.
import { describe, expect, it } from 'vitest';
import invalid from '../../tests/fixtures/invalid.html?raw';
import { type FetchLike, mapNu, type NuOptions, validateNu } from './nu';

const OPTIONS: NuOptions = {
  endpoint: 'http://localhost:8888/?out=json',
  source: '<!DOCTYPE html><title>x</title>',
  maxMessages: 100,
  maxText: 200,
  timeoutMs: 1000,
};

interface Sent {
  url: string;
  init: RequestInit;
}

function stub(status: number, body: string, sent: Sent[] = []): FetchLike {
  return async (url, init) => {
    sent.push({ url, init });
    return new Response(body, { status });
  };
}

const NU_ANSWER = {
  messages: [
    {
      type: 'error',
      lastLine: 10,
      firstColumn: 1,
      lastColumn: 15,
      message: 'Duplicate ID “intro”.',
      extract: '<p id="intro">Second',
    },
    {
      type: 'info',
      subType: 'warning',
      firstLine: 3,
      lastLine: 4,
      firstColumn: 7,
      lastColumn: 2,
      message: 'Consider adding a “lang” attribute.',
    },
    { type: 'info', lastLine: 5, lastColumn: 9, message: 'Trailing slash on void elements.' },
  ],
};

describe('validateNu', () => {
  it('posts the source as text/html to the endpoint, without credentials or redirects', async () => {
    const sent: Sent[] = [];
    await validateNu(OPTIONS, stub(200, JSON.stringify({ messages: [] }), sent));
    expect(sent).toHaveLength(1);
    expect(sent[0]?.url).toBe(OPTIONS.endpoint);
    const init = sent[0]?.init;
    expect(init?.method).toBe('POST');
    expect(init?.body).toBe(OPTIONS.source);
    expect(init?.headers).toEqual({ 'Content-Type': 'text/html; charset=utf-8' });
    expect(init?.credentials).toBe('omit');
    expect(init?.redirect).toBe('error');
    expect(init?.referrerPolicy).toBe('no-referrer');
  });

  it('maps the Nu messages to the runner shape', async () => {
    const answer = await validateNu(OPTIONS, stub(200, JSON.stringify(NU_ANSWER)));
    expect(answer).toEqual({
      engine: 'nu',
      version: '',
      truncated: false,
      messages: [
        {
          rule: 'nu-error',
          severity: 'error',
          message: 'Duplicate ID “intro”.',
          line: 10,
          column: 1,
          selector: null,
          extract: '<p id="intro">Second',
        },
        {
          rule: 'nu-warning',
          severity: 'warning',
          message: 'Consider adding a “lang” attribute.',
          line: 3,
          column: 7,
          selector: null,
          extract: '',
        },
        {
          rule: 'nu-info',
          severity: 'info',
          message: 'Trailing slash on void elements.',
          line: 5,
          column: 9,
          selector: null,
          extract: '',
        },
      ],
    });
  });

  it('fails with the status when the address answers with an error', async () => {
    await expect(validateNu(OPTIONS, stub(503, 'busy'))).resolves.toEqual({
      engine: 'nu',
      error: 'HTTP 503',
    });
  });

  it('fails when the answer is no Nu JSON', async () => {
    const failed = await validateNu(OPTIONS, stub(200, '<html>login</html>'));
    expect(failed).toMatchObject({ engine: 'nu' });
    expect('error' in failed && failed.error).toMatch(/^no Nu JSON/);
  });

  it('marks a request the browser refused as blocked, with the reason', async () => {
    // A network error (CSP connect-src, mixed content, local network access, CORS, no answer).
    const refused: FetchLike = async () => {
      throw new TypeError('Failed to fetch');
    };
    await expect(validateNu(OPTIONS, refused)).resolves.toEqual({
      engine: 'nu',
      error: 'Failed to fetch',
      blocked: true,
    });
  });

  it('fails when the address does not answer in time', async () => {
    const hanging: FetchLike = (_url, init) =>
      new Promise((_resolve, reject) => {
        init.signal?.addEventListener('abort', () => reject(new Error('aborted')));
      });
    await expect(validateNu({ ...OPTIONS, timeoutMs: 10 }, hanging)).resolves.toEqual({
      engine: 'nu',
      error: 'timeout',
    });
  });

  it('turns a non-document error of the checker into a failure', async () => {
    const body = JSON.stringify({
      messages: [{ type: 'non-document-error', subType: 'io', message: 'Unreadable input.' }],
    });
    await expect(validateNu(OPTIONS, stub(200, body))).resolves.toEqual({
      engine: 'nu',
      error: 'Unreadable input.',
    });
  });
});

describe('mapNu', () => {
  it('keeps to maxMessages and cuts long texts', () => {
    const long = 'x'.repeat(50);
    const answer = mapNu(
      { messages: [...NU_ANSWER.messages, { type: 'error', message: long, extract: long }] },
      { maxMessages: 3, maxText: 10 },
    );
    expect(answer.messages).toHaveLength(3);
    expect(answer.truncated).toBe(true);
    const cut = mapNu({ messages: [{ type: 'error', message: long, extract: long }] }, OPTIONS);
    expect(cut.messages[0]?.line).toBe(0);
    const short = mapNu({ messages: [{ type: 'error', message: long }] }, { maxMessages: 9, maxText: 10 });
    expect(short.messages[0]?.message).toHaveLength(10);
  });

  it('skips entries that are no message', () => {
    const answer = mapNu({ messages: [null, 7, { type: 'error' }] }, OPTIONS);
    expect(answer.messages).toEqual([
      {
        rule: 'nu-error',
        severity: 'error',
        message: '',
        line: 0,
        column: 0,
        selector: null,
        extract: '',
      },
    ]);
  });

  it('names the element at the end of each range, in the source it is given (RB-094-06)', () => {
    const answer = mapNu(
      {
        messages: [
          // The duplicate id of invalid.html: Nu marks the start tag, `<` to `>`.
          { type: 'error', lastLine: 10, firstColumn: 1, lastColumn: 15, message: 'Duplicate ID' },
          { type: 'error', message: 'No place' },
        ],
      },
      { ...OPTIONS, source: invalid },
    );
    expect(answer.messages.map((message) => message.selector)).toEqual([
      'html > body > main > p:nth-child(2)',
      null,
    ]);
    expect(answer.messages[0]).toMatchObject({ line: 10, column: 1 });
  });
});
