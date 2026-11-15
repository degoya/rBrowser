// The WebKit browsers of the sidecar and their views (RB-040-04): one headed WebKit per view,
// launched with the view (RB-080-40: the window is then the process's, so rBrowser binds it to
// the view's grid cell by the process id alone); per view a browser context with the
// breakpoint's options and one page - its own window. Events go to rBrowser as notifications:
//
// - `webkit.event { view, kind, url?, crashed? }`: `navigated` (main frame committed), `load`
//   and `closed` (the window was closed, its page crashed or its browser ended).
// - `webkit.agent { view, top, payload }`: a report of the view's sync agent, as the agent wrote
//   it; rBrowser parses it as untrusted input.

import { ErrorCode, RpcError } from '../rpc.ts';
import { type AgentNames, agentScript, reportPayload, runAgentCommand } from './agent.ts';
import { type CookieOutcome, addEach } from './cookies.ts';
import { InputState, type InputEvent, dispatch } from './input.ts';
import { type ViewOptions, contextOptions, mediaOptions, needsNewContext } from './params.ts';
import type { BrowserLike, ContextLike, CookieLike, LauncherLike, PageLike, ScreenshotScale } from './types.ts';

export const EVENT = 'webkit.event';
export const AGENT = 'webkit.agent';

export interface ServiceDeps {
  /** playwright-core's `webkit`, loaded on first use. */
  launcher(): Promise<LauncherLike>;
  /** The pinned build's executable; throws `WebkitNotInstalled` when it is not installed. */
  executablePath(): string;
  notify(method: string, params: Record<string, unknown>): void;
  names(): AgentNames;
}

interface View {
  /** The view's own browser process. */
  browser: BrowserLike;
  context: ContextLike;
  page: PageLike;
  options: ViewOptions;
  input: InputState;
  agentConfig: unknown;
  /** Set while rBrowser closes or rebuilds the view: its `close` is no exit. */
  closing: boolean;
}

/** What `create` answers: the browser process of the view (the owner of its window) and the
 *  WebKit version. */
export interface Created {
  pid: number | null;
  version: string;
}

export class WebkitService {
  readonly #deps: ServiceDeps;
  readonly #views = new Map<number, View>();
  /** Per view, the tail of its opens, rebuilds and closes: one at a time, never interleaved. */
  readonly #queues = new Map<number, Promise<void>>();
  #agent: string | null = null;
  #names: AgentNames | null = null;
  #version: string | null = null;

  constructor(deps: ServiceDeps) {
    this.#deps = deps;
  }

  /** Makes sure WebKit can start - the pinned build is installed, playwright-core loads - and
   *  takes `agent`, the sync agent bundle for every view. Starts no browser: each view gets one
   *  of its own with `create`. Answers the version of the last browser started, null before the
   *  first. */
  async launch(agent: string | null): Promise<{ version: string | null }> {
    if (agent !== null) this.#agent = agent;
    this.#deps.executablePath();
    await this.#deps.launcher();
    return { version: this.#version };
  }

  /** Opens view `view` in a browser of its own (closing an older one of that id) and loads
   *  `url` unless it is empty. A load that fails leaves the window on its error page, as a
   *  browser does. Answers the browser's process id, the owner of the view's window. */
  create(view: number, url: string | null, options: ViewOptions, agentConfig: unknown): Promise<Created> {
    return this.#serial(view, async () => {
      await this.#closeNow(view);
      const state = await this.#open(view, options, agentConfig, new InputState(), null);
      this.#views.set(view, state);
      if (url !== null) await this.#load(view, state.page, url);
      return { pid: processId(state.browser), version: state.browser.version() };
    });
  }

  async navigate(view: number, url: string): Promise<void> {
    await this.#view(view).page.goto(url, { waitUntil: 'commit' });
  }

  async reload(view: number): Promise<void> {
    await this.#view(view).page.reload({ waitUntil: 'commit' });
  }

  async back(view: number): Promise<void> {
    await this.#view(view).page.goBack({ waitUntil: 'commit' });
  }

  async forward(view: number): Promise<void> {
    await this.#view(view).page.goForward({ waitUntil: 'commit' });
  }

  /** Applies `options`: live where WebKit allows, else in a new context with the same URL (the
   *  window is a new one then, in the same process). A context that cannot be rebuilt ends the
   *  view as crashed. */
  emulate(view: number, options: ViewOptions): Promise<{ recreated: boolean }> {
    return this.#serial(view, () => this.#emulateNow(view, options));
  }

  async #emulateNow(view: number, options: ViewOptions): Promise<{ recreated: boolean }> {
    const state = this.#view(view);
    if (needsNewContext(state.options, options)) {
      await this.#rebuild(view, state, options);
      return { recreated: true };
    }
    const before = state.options;
    if (before.viewport.width !== options.viewport.width || before.viewport.height !== options.viewport.height) {
      await state.page.setViewportSize(options.viewport);
    }
    await state.page.emulateMedia(mediaOptions(options));
    if ((before.offline ?? false) !== (options.offline ?? false)) {
      await state.context.setOffline(options.offline ?? false);
    }
    await state.context.setExtraHTTPHeaders(options.extraHTTPHeaders ?? {});
    state.options = options;
    return { recreated: false };
  }

  /** Replays `events`; `before` runs in the agent first (the replay marker), and with
   *  `blockSubmitMs` a form submit in the view is prevented that long (leader-only submit). */
  async input(view: number, events: InputEvent[], before: unknown, blockSubmitMs: number): Promise<void> {
    const state = this.#view(view);
    if (before !== undefined && before !== null) {
      await state.page.evaluate(runAgentCommand, [this.#agentName(), before] as const);
    }
    if (blockSubmitMs > 0) await state.page.evaluate(blockSubmits, blockSubmitMs);
    for (const event of events) await dispatch(state.page, state.input, event);
  }

  /** Scrolls the document to the leader's position, scaled to this view's scroll range. */
  async scroll(view: number, position: ScrollPosition): Promise<void> {
    await this.#view(view).page.evaluate(scrollDocument, position);
  }

  /** A PNG of the viewport or the whole page; `scale` `css` gives one pixel per CSS px (the
   *  breakpoint's size whatever the cell's factor, RB-080-40), `device` or null the display's. */
  async screenshot(view: number, fullPage: boolean, scale: ScreenshotScale | null): Promise<{ pngBase64: string }> {
    const page = this.#view(view).page;
    const png = await page.screenshot(scale === null ? { fullPage, type: 'png' } : { fullPage, type: 'png', scale });
    return { pngBase64: Buffer.from(png).toString('base64') };
  }

  /** Runs one agent command (`rb_sync::AgentCommand`) in the view's main frame. */
  async agent(view: number, command: unknown): Promise<unknown> {
    return await this.#view(view).page.evaluate(runAgentCommand, [this.#agentName(), command] as const);
  }

  /** Adds `cookies` to the view's context (RB-040-05), after any open or rebuild of it. */
  addCookies(view: number, cookies: CookieLike[]): Promise<CookieOutcome> {
    return this.#serial(view, () => addEach(this.#view(view).context, cookies));
  }

  async raise(view: number): Promise<void> {
    await this.#view(view).page.bringToFront();
  }

  /** Closes view `view` and its browser; an unknown view is closed already. */
  close(view: number): Promise<void> {
    return this.#serial(view, () => this.#closeNow(view));
  }

  async #closeNow(view: number): Promise<void> {
    const state = this.#views.get(view);
    if (state === undefined) return;
    this.#views.delete(view);
    state.closing = true;
    await state.context.close().catch(() => {});
    await state.browser.close().catch(() => {});
  }

  /** Closes every view, each with its browser. */
  async shutdown(): Promise<void> {
    for (const view of [...this.#views.keys()]) await this.close(view);
  }

  views(): number[] {
    return [...this.#views.keys()].sort((a, b) => a - b);
  }

  /** Runs `run` after every earlier open, rebuild or close of `view`. */
  #serial<T>(view: number, run: () => Promise<T>): Promise<T> {
    const before = this.#queues.get(view) ?? Promise.resolve();
    const result = before.then(run);
    const tail = result.then(
      () => {},
      () => {},
    );
    this.#queues.set(view, tail);
    void tail.then(() => {
      if (this.#queues.get(view) === tail) this.#queues.delete(view);
    });
    return result;
  }

  /** `view` in a new context with `options`, on the page it showed, in its own browser. */
  async #rebuild(view: number, state: View, options: ViewOptions): Promise<void> {
    const url = state.page.url();
    // The new context keeps the old one's cookies (a login, RB-040-05's copies).
    const jar = await state.context.cookies().catch(() => []);
    state.closing = true;
    this.#views.delete(view);
    await state.context.close().catch(() => {});
    let fresh: View;
    try {
      fresh = await this.#open(view, options, state.agentConfig, state.input, state.browser);
    } catch (error) {
      await state.browser.close().catch(() => {});
      this.#deps.notify(EVENT, { view, kind: 'closed', crashed: true });
      throw error;
    }
    this.#views.set(view, fresh);
    if (jar.length > 0) await addEach(fresh.context, jar);
    if (url !== '' && url !== 'about:blank') await this.#load(view, fresh.page, url);
  }

  /** Loads `url`; a failure leaves the error page and is logged, not answered. */
  async #load(view: number, page: PageLike, url: string): Promise<void> {
    try {
      await page.goto(url, { waitUntil: 'commit' });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      process.stderr.write(`webkit view ${view}: loading failed: ${message}\n`);
    }
  }

  #view(view: number): View {
    const state = this.#views.get(view);
    if (state === undefined) {
      throw new RpcError(ErrorCode.UnknownView, `no WebKit view ${view}`);
    }
    return state;
  }

  #agentName(): string {
    return this.#names?.agent ?? '';
  }

  /** A browser of its own for `view`. */
  async #launch(view: number): Promise<BrowserLike> {
    const executablePath = this.#deps.executablePath();
    const launcher = await this.#deps.launcher();
    let browser: BrowserLike;
    try {
      browser = await launcher.launch({ headless: false, executablePath });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      throw new RpcError(ErrorCode.WebkitLaunchFailed, `WebKit did not start: ${message}`);
    }
    this.#names ??= this.#deps.names();
    this.#version = browser.version();
    browser.on('disconnected', () => this.#disconnected(view, browser));
    return browser;
  }

  /** `browser`, the process of `view`, ended: the view with it, as a crash unless rBrowser
   *  closed it. */
  #disconnected(view: number, browser: BrowserLike): void {
    const state = this.#views.get(view);
    if (state === undefined || state.browser !== browser) return;
    this.#views.delete(view);
    if (state.closing) return;
    state.closing = true;
    this.#deps.notify(EVENT, { view, kind: 'closed', crashed: true });
  }

  /** A context with `options` for `view` in `browser`, or in a new browser of its own; a
   *  failed set-up closes what was opened here. */
  async #open(
    view: number,
    options: ViewOptions,
    agentConfig: unknown,
    input: InputState,
    browser: BrowserLike | null,
  ): Promise<View> {
    const own = browser === null;
    const running = browser ?? (await this.#launch(view));
    let context: ContextLike;
    try {
      context = await running.newContext(contextOptions(options));
    } catch (error) {
      if (own) await running.close().catch(() => {});
      throw error;
    }
    try {
      return await this.#setUp(running, context, view, options, agentConfig, input);
    } catch (error) {
      await context.close().catch(() => {});
      if (own) await running.close().catch(() => {});
      throw error;
    }
  }

  async #setUp(
    browser: BrowserLike,
    context: ContextLike,
    view: number,
    options: ViewOptions,
    agentConfig: unknown,
    input: InputState,
  ): Promise<View> {
    const names = this.#names;
    let page: PageLike | null = null;
    if (this.#agent !== null && names !== null) {
      await context.exposeBinding(names.binding, (source, payload) => {
        const text = reportPayload(payload);
        if (text === null || page === null) return;
        this.#deps.notify(AGENT, { view, top: source.frame === page.mainFrame(), payload: text });
      });
      await context.addInitScript({ content: agentScript(this.#agent, agentConfig, names) });
    }
    page = await context.newPage();
    const state: View = { browser, context, page, options, input, agentConfig, closing: false };
    const opened = page;
    opened.on('framenavigated', (frame) => {
      if (frame === opened.mainFrame()) this.#deps.notify(EVENT, { view, kind: 'navigated', url: frame.url() });
    });
    opened.on('load', () => this.#deps.notify(EVENT, { view, kind: 'load', url: opened.url() }));
    opened.on('close', () => this.#ended(view, state, false));
    opened.on('crash', () => this.#ended(view, state, true));
    await opened.emulateMedia(mediaOptions(options));
    return state;
  }

  #ended(view: number, state: View, crashed: boolean): void {
    if (state.closing) return;
    state.closing = true;
    if (this.#views.get(view) === state) this.#views.delete(view);
    this.#deps.notify(EVENT, { view, kind: 'closed', crashed });
    // The window is gone: its process goes with it.
    void state.browser.close().catch(() => {});
  }
}

/** The process id of `browser`, null when Playwright does not know it. */
function processId(browser: BrowserLike): number | null {
  const pid = browser.process?.()?.pid;
  return typeof pid === 'number' ? pid : null;
}

/** The leader's scroll position and range. */
export interface ScrollPosition {
  x: number;
  y: number;
  maxX: number;
  maxY: number;
}

/** In the page: scrolls the document to the same share of its range as the leader's. */
function scrollDocument(position: ScrollPosition): void {
  const page = globalThis as unknown as {
    document: { scrollingElement: { scrollWidth: number; scrollHeight: number } | null };
    innerWidth: number;
    innerHeight: number;
    scrollTo(x: number, y: number): void;
  };
  const root = page.document.scrollingElement;
  const rangeX = root === null ? 0 : Math.max(0, root.scrollWidth - page.innerWidth);
  const rangeY = root === null ? 0 : Math.max(0, root.scrollHeight - page.innerHeight);
  const share = (value: number, max: number, range: number) => (max > 0 ? (value / max) * range : value);
  page.scrollTo(share(position.x, position.maxX, rangeX), share(position.y, position.maxY, rangeY));
}

/** In the page: prevents every form submit for `ms` milliseconds. */
function blockSubmits(ms: number): void {
  const page = globalThis as unknown as {
    addEventListener(type: string, listener: (event: { preventDefault(): void }) => void, capture: boolean): void;
    removeEventListener(type: string, listener: (event: { preventDefault(): void }) => void, capture: boolean): void;
    setTimeout(callback: () => void, ms: number): unknown;
  };
  const block = (event: { preventDefault(): void }) => event.preventDefault();
  page.addEventListener('submit', block, true);
  page.setTimeout(() => page.removeEventListener('submit', block, true), ms);
}
