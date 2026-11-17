import { describe, expect, it } from 'vitest';

import { ErrorCode } from '../rpc.ts';
import { Router } from '../router.ts';
import { cookies } from './cookies.ts';
import { FakeLauncher } from './fake.ts';
import { registerWebkit } from './index.ts';
import { WebkitService } from './service.ts';

const SID = {
  name: 'sid',
  value: 'hunter2',
  domain: '127.0.0.1',
  path: '/',
  expires: -1,
  httpOnly: true,
  secure: false,
  sameSite: 'Lax',
};

const PHONE = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };

function setup() {
  const launcher = new FakeLauncher();
  const service = new WebkitService({
    launcher: async () => launcher,
    executablePath: () => 'C:/pw/webkit/Playwright.exe',
    notify: () => {},
    names: () => ({ binding: '__rbBINDING', agent: '__rbAGENT' }),
  });
  const router = new Router();
  registerWebkit(router, service);
  const ctx = { signal: new AbortController().signal, progress: () => {} };
  const call = (method: string, params: unknown) => router.call(method, params, ctx);
  return { call, browser: launcher.browser };
}

describe('cookies', () => {
  it('takes the cookies as Playwright adds them', () => {
    expect(cookies([SID, { ...SID, name: 'theme', expires: 1_900_000_000, sameSite: 'None', secure: true }])).toEqual([
      SID,
      { ...SID, name: 'theme', expires: 1_900_000_000, sameSite: 'None', secure: true },
    ]);
  });

  it('refuses a malformed cookie without quoting its value', () => {
    const bad = [
      { ...SID, value: 42 },
      { ...SID, domain: '.' },
      { ...SID, path: 'account' },
      { ...SID, sameSite: 'lax' },
      { ...SID, expires: Number.NaN },
      { ...SID, httpOnly: 'yes' },
      { ...SID, value: 'hunter2'.repeat(1000) },
    ];
    for (const entry of bad) {
      let message = '';
      try {
        cookies([entry]);
      } catch (error) {
        expect(error).toMatchObject({ code: ErrorCode.InvalidParams });
        message = String((error as Error).message);
      }
      expect(message).not.toBe('');
      expect(message).not.toContain('hunter2');
    }
    expect(() => cookies('sid=1')).toThrow();
  });
});

describe('webkit.view.cookies', () => {
  it('adds the cookies to the view context and counts what WebKit refused', async () => {
    const { call, browser } = setup();
    await call('webkit.view.create', { view: 4, options: PHONE });
    browser.refusedCookie = 'bad';
    const answer = await call('webkit.view.cookies', { view: 4, cookies: [SID, { ...SID, name: 'bad' }] });
    expect(answer).toEqual({ added: 1, refused: 1 });
    expect(JSON.stringify(answer)).not.toContain('hunter2');
    expect(browser.contexts[0]?.jar).toEqual([SID]);
  });

  it('answers an unknown view as such', async () => {
    const { call } = setup();
    await expect(call('webkit.view.cookies', { view: 9, cookies: [SID] })).rejects.toMatchObject({
      code: ErrorCode.UnknownView,
    });
  });

  it('keeps the cookies when the view moves into a new context', async () => {
    const { call, browser } = setup();
    await call('webkit.view.create', { view: 1, url: 'http://127.0.0.1:18766/', options: PHONE });
    await call('webkit.view.cookies', { view: 1, cookies: [SID] });
    const answer = await call('webkit.view.emulate', { view: 1, options: { ...PHONE, isMobile: false } });
    expect(answer).toEqual({ recreated: true });
    expect(browser.contexts).toHaveLength(2);
    expect(browser.contexts[1]?.jar).toEqual([SID]);
  });
});
