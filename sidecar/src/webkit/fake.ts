// A fake playwright-core for the WebKit module's tests: it records every call and lets a test
// fire the page's events. Not part of the bundle (only tests import it).

import type {
  BindingSource,
  BrowserLike,
  ContextLike,
  ContextOptions,
  CookieLike,
  FrameLike,
  LaunchOptions,
  LauncherLike,
  MediaOptions,
  PageLike,
} from './types.ts';

type Listener = (frame: FrameLike) => void;

export class FakeFrame implements FrameLike {
  current = 'about:blank';
  url(): string {
    return this.current;
  }
}

export class FakePage implements PageLike {
  readonly calls: string[] = [];
  readonly frame = new FakeFrame();
  readonly listeners = new Map<string, Listener[]>();
  /** What `evaluate` answers; the function and argument go to `evaluated`. */
  evaluateAnswer: unknown = undefined;
  readonly evaluated: unknown[] = [];
  history: string[] = [];
  readonly mouse = {
    move: async (x: number, y: number) => void this.calls.push(`mouse.move ${x} ${y}`),
    down: async (o: { button: string; clickCount: number }) => void this.calls.push(`mouse.down ${o.button} ${o.clickCount}`),
    up: async (o: { button: string; clickCount: number }) => void this.calls.push(`mouse.up ${o.button} ${o.clickCount}`),
    wheel: async (dx: number, dy: number) => void this.calls.push(`mouse.wheel ${dx} ${dy}`),
  };
  readonly keyboard = {
    down: async (key: string) => void this.calls.push(`key.down ${key}`),
    up: async (key: string) => void this.calls.push(`key.up ${key}`),
    insertText: async (text: string) => void this.calls.push(`key.insertText ${text}`),
  };

  /** When set, `goto` fails with it after the commit (an error page). */
  gotoFailure: string | null = null;

  async goto(url: string): Promise<unknown> {
    this.calls.push(`goto ${url}`);
    if (this.gotoFailure !== null) throw new Error(this.gotoFailure);
    this.frame.current = url;
    this.history.push(url);
    this.fire('framenavigated', this.frame);
    return null;
  }
  async reload(): Promise<unknown> {
    this.calls.push('reload');
    return null;
  }
  async goBack(): Promise<unknown> {
    this.calls.push('back');
    return null;
  }
  async goForward(): Promise<unknown> {
    this.calls.push('forward');
    return null;
  }
  url(): string {
    return this.frame.current;
  }
  mainFrame(): FrameLike {
    return this.frame;
  }
  async setViewportSize(size: { width: number; height: number }): Promise<void> {
    this.calls.push(`viewport ${size.width}x${size.height}`);
  }
  async emulateMedia(options: MediaOptions): Promise<void> {
    this.calls.push(`media ${JSON.stringify(options)}`);
  }
  async screenshot(options: { fullPage: boolean; scale?: 'css' | 'device' }): Promise<Uint8Array> {
    this.calls.push(options.scale === undefined ? `screenshot ${options.fullPage}` : `screenshot ${options.fullPage} ${options.scale}`);
    return Uint8Array.from([137, 80, 78, 71]);
  }
  async evaluate<R, A>(fn: (arg: A) => R, arg: A): Promise<Awaited<R>> {
    this.calls.push(`evaluate ${fn.name}`);
    this.evaluated.push(arg);
    return this.evaluateAnswer as Awaited<R>;
  }
  async bringToFront(): Promise<void> {
    this.calls.push('bringToFront');
  }
  on(event: string, listener: Listener): unknown {
    const list = this.listeners.get(event) ?? [];
    list.push(listener);
    this.listeners.set(event, list);
    return this;
  }
  fire(event: string, frame: FrameLike = this.frame): void {
    for (const listener of this.listeners.get(event) ?? []) listener(frame);
  }
}

export class FakeContext implements ContextLike {
  readonly calls: string[] = [];
  readonly pages: FakePage[] = [];
  readonly scripts: string[] = [];
  binding: ((source: BindingSource, payload: unknown) => void) | null = null;
  bindingName = '';
  closed = false;
  readonly options: ContextOptions;
  readonly #browser: FakeBrowser;

  constructor(options: ContextOptions, browser: FakeBrowser) {
    this.options = options;
    this.#browser = browser;
  }
  async newPage(): Promise<PageLike> {
    const page = new FakePage();
    page.gotoFailure = this.#browser.gotoFailure;
    this.pages.push(page);
    return page;
  }
  async addInitScript(script: { content: string }): Promise<void> {
    this.scripts.push(script.content);
  }
  async exposeBinding(name: string, callback: (source: BindingSource, payload: unknown) => void): Promise<void> {
    if (this.#browser.bindingFailure !== null) throw new Error(this.#browser.bindingFailure);
    this.bindingName = name;
    this.binding = callback;
  }
  async setOffline(offline: boolean): Promise<void> {
    this.calls.push(`offline ${offline}`);
  }
  async setExtraHTTPHeaders(headers: Record<string, string>): Promise<void> {
    this.calls.push(`headers ${JSON.stringify(headers)}`);
  }
  /** The context's cookie jar. */
  readonly jar: CookieLike[] = [];
  async addCookies(cookies: CookieLike[]): Promise<void> {
    for (const cookie of cookies) {
      if (cookie.name === this.#browser.refusedCookie) throw new Error(`refused ${cookie.name}=${cookie.value}`);
      this.jar.push(cookie);
    }
  }
  async cookies(): Promise<CookieLike[]> {
    return [...this.jar];
  }
  async close(): Promise<void> {
    this.closed = true;
    for (const page of this.pages) page.fire('close');
  }
}

export class FakeBrowser implements BrowserLike {
  readonly contexts: FakeContext[] = [];
  closed = false;
  /** When set, new pages' `goto` fails with it. */
  gotoFailure: string | null = null;
  /** When set, `exposeBinding` fails with it. */
  bindingFailure: string | null = null;
  /** When set, `newContext` fails with it. */
  contextFailure: string | null = null;
  /** When set, `addCookies` refuses the cookie of this name. */
  refusedCookie: string | null = null;
  #disconnected: Array<() => void> = [];

  async newContext(options: ContextOptions): Promise<ContextLike> {
    // A real context takes a while: overlapping calls interleave here.
    await new Promise((resolve) => setTimeout(resolve, 0));
    if (this.contextFailure !== null) throw new Error(this.contextFailure);
    const context = new FakeContext(options, this);
    this.contexts.push(context);
    return context;
  }
  async close(): Promise<void> {
    this.closed = true;
  }
  version(): string {
    return '27.2';
  }
  /** The process id `process()` answers; null is a browser without one. */
  pid: number | null = null;
  process(): { pid?: number } | null {
    return this.pid === null ? null : { pid: this.pid };
  }
  on(_event: 'disconnected', listener: () => void): unknown {
    this.#disconnected.push(listener);
    return this;
  }
  /** The browser process ended. */
  crash(): void {
    for (const listener of this.#disconnected) listener();
  }
  /** The last page of the newest context. */
  page(): FakePage {
    const page = this.contexts.at(-1)?.pages.at(-1);
    if (page === undefined) throw new Error('no page');
    return page;
  }
}

export class FakeLauncher implements LauncherLike {
  readonly launches: LaunchOptions[] = [];
  /** The browser every launch answers, unless `spawn` makes one per launch. */
  readonly browser = new FakeBrowser();
  /** Every browser launched, in order. */
  readonly browsers: FakeBrowser[] = [];
  /** When set, each launch is a browser of its own (a process per view, RB-080-40). */
  spawn: (() => FakeBrowser) | null = null;
  fail: string | null = null;

  async launch(options: LaunchOptions): Promise<BrowserLike> {
    this.launches.push(options);
    if (this.fail !== null) throw new Error(this.fail);
    const browser = this.spawn === null ? this.browser : this.spawn();
    this.browsers.push(browser);
    return browser;
  }
}
