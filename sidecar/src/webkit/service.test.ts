import { describe, expect, it } from 'vitest';

import { ErrorCode, RpcError } from '../rpc.ts';
import { Router } from '../router.ts';
import { FakeBrowser, FakeLauncher } from './fake.ts';
import { registerWebkit } from './index.ts';
import { AGENT, EVENT, WebkitService } from './service.ts';

const PHONE = {
  viewport: { width: 390, height: 844 },
  deviceScaleFactor: 3,
  isMobile: true,
  hasTouch: true,
  reducedMotion: 'reduce',
};

function setup(options: { installed?: boolean } = {}) {
  const launcher = new FakeLauncher();
  const notes: Array<[string, Record<string, unknown>]> = [];
  const service = new WebkitService({
    launcher: async () => launcher,
    executablePath: () => {
      if (options.installed === false) throw new RpcError(ErrorCode.WebkitNotInstalled, 'WebKit 2388 is not installed');
      return 'C:/pw/webkit-2388/Playwright.exe';
    },
    notify: (method, params) => notes.push([method, params]),
    names: () => ({ binding: '__rbBINDING', agent: '__rbAGENT' }),
  });
  const router = new Router();
  registerWebkit(router, service);
  const ctx = { signal: new AbortController().signal, progress: () => {} };
  const call = (method: string, params: unknown) => router.call(method, params, ctx);
  return { launcher, notes, service, call, browser: launcher.browser };
}

describe('webkit.launch', () => {
  it('checks the build and loads playwright-core, but starts no browser before a view', async () => {
    const { call, launcher } = setup();
    expect(await call('webkit.launch', { agent: 'void 0;' })).toEqual({ version: null });
    await call('webkit.launch', {});
    expect(launcher.launches).toEqual([]);
    await call('webkit.view.create', { view: 1, options: PHONE });
    expect(await call('webkit.launch', {})).toEqual({ version: '27.2' });
  });

  it('says when the build is not installed', async () => {
    const { call, launcher } = setup({ installed: false });
    await expect(call('webkit.launch', {})).rejects.toMatchObject({ code: ErrorCode.WebkitNotInstalled });
    await expect(call('webkit.view.create', { view: 1, options: PHONE })).rejects.toMatchObject({
      code: ErrorCode.WebkitNotInstalled,
    });
    expect(launcher.launches).toEqual([]);
  });

  it('reports a failed start and tries again next time', async () => {
    const { call, launcher, service } = setup();
    launcher.fail = 'spawn failed';
    await expect(call('webkit.view.create', { view: 1, options: PHONE })).rejects.toMatchObject({
      code: ErrorCode.WebkitLaunchFailed,
    });
    expect(service.views()).toEqual([]);
    launcher.fail = null;
    await expect(call('webkit.view.create', { view: 1, options: PHONE })).resolves.toMatchObject({ version: '27.2' });
    expect(launcher.launches).toEqual([
      { headless: false, executablePath: 'C:/pw/webkit-2388/Playwright.exe' },
      { headless: false, executablePath: 'C:/pw/webkit-2388/Playwright.exe' },
    ]);
  });
});

describe('webkit.view.create', () => {
  it('opens a context with the breakpoint options and loads the URL', async () => {
    const { call, browser, notes } = setup();
    browser.pid = 4242;
    expect(await call('webkit.view.create', { view: 3, url: 'http://localhost/', options: PHONE })).toEqual({
      pid: 4242,
      version: '27.2',
    });
    const context = browser.contexts[0];
    expect(context?.options).toMatchObject({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true });
    expect(browser.page().calls).toContain('goto http://localhost/');
    expect(notes).toContainEqual([EVENT, { view: 3, kind: 'navigated', url: 'http://localhost/' }]);
  });

  it('injects the agent as an init script and forwards its reports', async () => {
    const { call, browser, notes } = setup();
    await call('webkit.launch', { agent: 'globalThis.__rbAgentLoaded = 1;' });
    await call('webkit.view.create', { view: 1, options: PHONE, agentConfig: { active: true } });
    const context = browser.contexts[0];
    expect(context?.scripts[0]).toContain('globalThis.__rbAgentLoaded = 1;');
    expect(context?.bindingName).toBe('__rbBINDING');
    const page = browser.page();
    context?.binding?.({ frame: page.mainFrame() }, '{"kind":"ready"}');
    context?.binding?.({ frame: { url: () => 'http://frame/' } }, '{"kind":"event"}');
    context?.binding?.({ frame: page.mainFrame() }, 42);
    expect(notes.filter(([method]) => method === AGENT)).toEqual([
      [AGENT, { view: 1, top: true, payload: '{"kind":"ready"}' }],
      [AGENT, { view: 1, top: false, payload: '{"kind":"event"}' }],
    ]);
  });

  it('gives every view a browser process of its own, so its window is the process\'s (RB-080-40)', async () => {
    const { call, launcher, service } = setup();
    let next = 100;
    launcher.spawn = () => {
      const browser = new FakeBrowser();
      browser.pid = next++;
      return browser;
    };
    expect(await call('webkit.view.create', { view: 1, options: PHONE })).toEqual({ pid: 100, version: '27.2' });
    expect(await call('webkit.view.create', { view: 2, options: PHONE })).toEqual({ pid: 101, version: '27.2' });
    expect(launcher.launches).toHaveLength(2);
    // A rebuilt context stays in the view's process: the window is new, the process is not.
    expect(await call('webkit.view.emulate', { view: 1, options: { ...PHONE, deviceScaleFactor: 2 } })).toEqual({
      recreated: true,
    });
    expect(launcher.launches).toHaveLength(2);
    expect(launcher.browsers[0]?.contexts).toHaveLength(2);
    // Closing a view closes its browser alone.
    await call('webkit.view.close', { view: 1 });
    expect(launcher.browsers[0]?.closed).toBe(true);
    expect(launcher.browsers[1]?.closed).toBe(false);
    expect(service.views()).toEqual([2]);
    // Its browser ending ends the view alone.
    launcher.browsers[1]?.crash();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(service.views()).toEqual([]);
  });

  it('answers a browser without a known process with null', async () => {
    const { call } = setup();
    expect(await call('webkit.view.create', { view: 1, options: PHONE })).toEqual({ pid: null, version: '27.2' });
  });

  it('keeps the window on its error page when the first load fails', async () => {
    const { call, browser, service } = setup();
    browser.gotoFailure = 'net::ERR_CONNECTION_REFUSED';
    await expect(call('webkit.view.create', { view: 1, url: 'http://localhost:1/', options: PHONE })).resolves.toMatchObject({
      version: '27.2',
    });
    expect(service.views()).toEqual([1]);
    expect(browser.contexts[0]?.closed).toBe(false);
  });

  it('closes the context and the browser of a view that could not be set up', async () => {
    const { call, browser, service } = setup();
    await call('webkit.launch', { agent: 'void 0;' });
    browser.bindingFailure = 'binding refused';
    await expect(call('webkit.view.create', { view: 1, options: PHONE })).rejects.toThrow('binding refused');
    expect(service.views()).toEqual([]);
    expect(browser.contexts.every((c) => c.closed)).toBe(true);
    expect(browser.closed).toBe(true);
  });

  it('refuses a URL scheme a view must not load', async () => {
    const { call } = setup();
    for (const url of ['javascript:alert(1)', 'data:text/html,x', 'relative/path']) {
      await expect(call('webkit.view.create', { view: 1, url, options: PHONE })).rejects.toMatchObject({
        code: ErrorCode.InvalidParams,
      });
    }
  });

  it('leaves the pixel ratio to the display when none is sent, and takes up to 10', async () => {
    const { call, browser } = setup();
    const { deviceScaleFactor: _dpr, ...display } = PHONE;
    await call('webkit.view.create', { view: 1, options: display });
    expect(browser.contexts[0]?.options).not.toHaveProperty('deviceScaleFactor');
    await call('webkit.view.create', { view: 2, options: { ...PHONE, deviceScaleFactor: 10 } });
    expect(browser.contexts[1]?.options.deviceScaleFactor).toBe(10);
  });

  it('opens the context in the time zone and at the position asked for', async () => {
    const { call, browser } = setup();
    const geolocation = { latitude: 52.52, longitude: 13.405, accuracy: 50 };
    await call('webkit.view.create', {
      view: 1,
      options: { ...PHONE, timezoneId: 'Europe/Berlin', geolocation, permissions: ['geolocation'] },
    });
    expect(browser.contexts[0]?.options).toMatchObject({
      timezoneId: 'Europe/Berlin',
      geolocation,
      permissions: ['geolocation'],
    });
  });

  it('refuses options out of range or of the wrong type', async () => {
    const { call } = setup();
    const bad = [
      { ...PHONE, viewport: { width: 0, height: 844 } },
      { ...PHONE, deviceScaleFactor: 'three' },
      { ...PHONE, deviceScaleFactor: 0 },
      { ...PHONE, deviceScaleFactor: 11 },
      { ...PHONE, geolocation: { latitude: 91, longitude: 0, accuracy: 1 } },
      { ...PHONE, geolocation: { latitude: 0, longitude: 0, accuracy: -1 } },
      { ...PHONE, permissions: ['camera'] },
      { ...PHONE, colorScheme: 'sepia' },
      { ...PHONE, extraHTTPHeaders: { 'X-Bad': 'a\r\nInjected: 1' } },
    ];
    for (const options of bad) {
      await expect(call('webkit.view.create', { view: 1, options })).rejects.toMatchObject({ code: ErrorCode.InvalidParams });
    }
  });
});

describe('a view', () => {
  it('navigates, reloads and walks the history', async () => {
    const { call, browser } = setup();
    await call('webkit.view.create', { view: 2, options: PHONE });
    await call('webkit.view.navigate', { view: 2, url: 'https://example.test/a' });
    await call('webkit.view.reload', { view: 2 });
    await call('webkit.view.back', { view: 2 });
    await call('webkit.view.forward', { view: 2 });
    await call('webkit.view.raise', { view: 2 });
    expect(browser.page().calls.filter((c) => !c.startsWith('media'))).toEqual([
      'goto https://example.test/a',
      'reload',
      'back',
      'forward',
      'bringToFront',
    ]);
  });

  it('answers an unknown view with its own error', async () => {
    const { call } = setup();
    await expect(call('webkit.view.navigate', { view: 9, url: 'http://x/' })).rejects.toMatchObject({ code: ErrorCode.UnknownView });
  });

  it('applies media, viewport and network live', async () => {
    const { call, browser } = setup();
    await call('webkit.view.create', { view: 1, options: PHONE });
    const answer = await call('webkit.view.emulate', {
      view: 1,
      options: { ...PHONE, viewport: { width: 400, height: 800 }, colorScheme: 'dark', offline: true, media: 'print' },
    });
    expect(answer).toEqual({ recreated: false });
    const page = browser.page();
    expect(page.calls).toContain('viewport 400x800');
    expect(page.calls).toContain(
      `media ${JSON.stringify({ colorScheme: 'dark', reducedMotion: 'reduce', forcedColors: null, contrast: null, media: 'print' })}`,
    );
    expect(browser.contexts[0]?.calls).toContain('offline true');
    expect(browser.contexts).toHaveLength(1);
  });

  it('builds a new context for an option WebKit cannot change live, on the same URL', async () => {
    const { call, browser, notes } = setup();
    await call('webkit.view.create', { view: 1, url: 'http://localhost/page', options: PHONE });
    const answer = await call('webkit.view.emulate', { view: 1, options: { ...PHONE, deviceScaleFactor: 2 } });
    expect(answer).toEqual({ recreated: true });
    expect(browser.contexts).toHaveLength(2);
    expect(browser.contexts[0]?.closed).toBe(true);
    expect(browser.contexts[1]?.options.deviceScaleFactor).toBe(2);
    expect(browser.page().calls).toContain('goto http://localhost/page');
    expect(notes.some(([, p]) => p.kind === 'closed')).toBe(false);
  });

  it('builds one context at a time when emulations overlap', async () => {
    const { call, browser, service } = setup();
    await call('webkit.view.create', { view: 1, url: 'http://localhost/page', options: PHONE });
    await Promise.all([
      call('webkit.view.emulate', { view: 1, options: { ...PHONE, deviceScaleFactor: 2 } }),
      call('webkit.view.emulate', { view: 1, options: { ...PHONE, deviceScaleFactor: 1 } }),
    ]);
    expect(browser.contexts.filter((c) => !c.closed)).toHaveLength(1);
    expect(browser.contexts.at(-1)?.options.deviceScaleFactor).toBe(1);
    expect(service.views()).toEqual([1]);
  });

  it('rebuilds the context for a new position', async () => {
    const { call, browser } = setup();
    const at = (latitude: number) => ({ latitude, longitude: 13.4, accuracy: 50 });
    await call('webkit.view.create', { view: 1, options: { ...PHONE, geolocation: at(52.5) } });
    const answer = await call('webkit.view.emulate', { view: 1, options: { ...PHONE, geolocation: at(48.1) } });
    expect(answer).toEqual({ recreated: true });
    expect(browser.contexts).toHaveLength(2);
  });

  it('ends the view as crashed when its new context cannot be built', async () => {
    const { call, browser, notes, service } = setup();
    await call('webkit.view.create', { view: 1, options: PHONE });
    browser.contextFailure = 'no more contexts';
    await expect(call('webkit.view.emulate', { view: 1, options: { ...PHONE, deviceScaleFactor: 2 } })).rejects.toThrow(
      'no more contexts',
    );
    expect(service.views()).toEqual([]);
    expect(notes).toContainEqual([EVENT, { view: 1, kind: 'closed', crashed: true }]);
  });

  it('replays input after the agent command and with a submit block', async () => {
    const { call, browser } = setup();
    await call('webkit.view.create', { view: 1, options: PHONE });
    await call('webkit.view.input', {
      view: 1,
      before: { cmd: 'replay', ms: 300 },
      blockSubmitMs: 1000,
      events: [
        { type: 'mouse', phase: 'down', x: 10, y: 20, button: 'left', clicks: 1, modifiers: 0 },
        { type: 'mouse', phase: 'up', x: 10, y: 20, button: 'left', clicks: 1, modifiers: 0 },
        { type: 'text', text: 'hello' },
      ],
    });
    const page = browser.page();
    expect(page.calls.filter((c) => !c.startsWith('media'))).toEqual([
      'evaluate runAgentCommand',
      'evaluate blockSubmits',
      'mouse.move 10 20',
      'mouse.down left 1',
      'mouse.up left 1',
      'key.insertText hello',
    ]);
    expect(page.evaluated[0]).toEqual(['__rbAGENT', { cmd: 'replay', ms: 300 }]);
    expect(page.evaluated[1]).toBe(1000);
  });

  it('scrolls, takes a screenshot and runs agent commands', async () => {
    const { call, browser } = setup();
    await call('webkit.view.create', { view: 1, options: PHONE });
    const page = browser.page();
    await call('webkit.view.scroll', { view: 1, x: 0, y: 500, maxX: 0, maxY: 1000 });
    expect(page.evaluated.at(-1)).toEqual({ x: 0, y: 500, maxX: 0, maxY: 1000 });
    expect(await call('webkit.view.screenshot', { view: 1, fullPage: true })).toEqual({ pngBase64: 'iVBORw==' });
    expect(page.calls).toContain('screenshot true');
    // In its cell the page is scaled (RB-080-40): a screenshot in CSS px keeps the breakpoint's size.
    await call('webkit.view.screenshot', { view: 1, scale: 'css' });
    expect(page.calls).toContain('screenshot false css');
    await expect(call('webkit.view.screenshot', { view: 1, scale: 'half' })).rejects.toMatchObject({ code: ErrorCode.InvalidParams });
    page.evaluateAnswer = { status: 'ok', x: 5, y: 6, scrolled: false };
    expect(await call('webkit.view.agent', { view: 1, command: { cmd: 'ping' } })).toEqual({
      status: 'ok',
      x: 5,
      y: 6,
      scrolled: false,
    });
  });

  it('reports a window closed by the user and a crash, but not its own close', async () => {
    const { call, browser, notes } = setup();
    await call('webkit.view.create', { view: 1, options: PHONE });
    browser.page().fire('close');
    await call('webkit.view.create', { view: 2, options: PHONE });
    browser.page().fire('crash');
    await call('webkit.view.create', { view: 3, options: PHONE });
    await call('webkit.view.close', { view: 3 });
    await call('webkit.view.close', { view: 3 });
    expect(notes.filter(([, p]) => p.kind === 'closed')).toEqual([
      [EVENT, { view: 1, kind: 'closed', crashed: false }],
      [EVENT, { view: 2, kind: 'closed', crashed: true }],
    ]);
  });

  it('reports the end of a view\'s browser as its crash and launches a new one for the next view', async () => {
    const { call, browser, notes, service, launcher } = setup();
    await call('webkit.view.create', { view: 1, options: PHONE });
    browser.crash();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(notes).toContainEqual([EVENT, { view: 1, kind: 'closed', crashed: true }]);
    expect(service.views()).toEqual([]);
    await call('webkit.view.create', { view: 1, options: PHONE });
    expect(launcher.launches).toHaveLength(2);
  });

  it('closes every view and its browser on shutdown', async () => {
    const { call, browser, service } = setup();
    await call('webkit.view.create', { view: 1, options: PHONE });
    await call('webkit.view.create', { view: 2, options: PHONE });
    await call('webkit.shutdown', null);
    expect(service.views()).toEqual([]);
    expect(browser.closed).toBe(true);
    expect(browser.contexts.every((c) => c.closed)).toBe(true);
  });

  it('refuses malformed input events', async () => {
    const { call } = setup();
    await call('webkit.view.create', { view: 1, options: PHONE });
    const bad = [
      [{ type: 'mouse', phase: 'down', x: 1, y: 1, button: 'back', clicks: 1, modifiers: 0 }],
      [{ type: 'mouse', phase: 'down', x: Number.NaN, y: 1, button: 'left', clicks: 1, modifiers: 0 }],
      [{ type: 'key', phase: 'press', key: 'a', code: 'KeyA', modifiers: 0 }],
      [{ type: 'drag' }],
      'click',
    ];
    for (const events of bad) {
      await expect(call('webkit.view.input', { view: 1, events })).rejects.toBeInstanceOf(RpcError);
    }
  });
});
