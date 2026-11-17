// The part of playwright-core the WebKit module uses (RB-040-04). The module talks to these
// interfaces only, so its tests run against a fake; `playwright.ts` adapts the real `webkit`.

export type MouseButton = 'left' | 'middle' | 'right';

export interface MouseLike {
  move(x: number, y: number): Promise<void>;
  down(options: { button: MouseButton; clickCount: number }): Promise<void>;
  up(options: { button: MouseButton; clickCount: number }): Promise<void>;
  wheel(deltaX: number, deltaY: number): Promise<void>;
}

export interface KeyboardLike {
  down(key: string): Promise<void>;
  up(key: string): Promise<void>;
  insertText(text: string): Promise<void>;
}

export interface FrameLike {
  url(): string;
}

/** `page.screenshot`'s scale: one pixel per CSS px, or per device px (the default). */
export type ScreenshotScale = 'css' | 'device';

/** The media `page.emulateMedia` sets; `null` switches an emulation off. */
export interface MediaOptions {
  colorScheme: 'light' | 'dark' | 'no-preference' | null;
  reducedMotion: 'reduce' | 'no-preference' | null;
  forcedColors: 'active' | 'none' | null;
  contrast: 'more' | 'no-preference' | null;
  media: 'screen' | 'print' | null;
}

export interface PageLike {
  readonly mouse: MouseLike;
  readonly keyboard: KeyboardLike;
  goto(url: string, options: { waitUntil: 'commit' }): Promise<unknown>;
  reload(options: { waitUntil: 'commit' }): Promise<unknown>;
  goBack(options: { waitUntil: 'commit' }): Promise<unknown>;
  goForward(options: { waitUntil: 'commit' }): Promise<unknown>;
  url(): string;
  mainFrame(): FrameLike;
  setViewportSize(size: { width: number; height: number }): Promise<void>;
  emulateMedia(options: MediaOptions): Promise<void>;
  screenshot(options: { fullPage: boolean; type: 'png'; scale?: ScreenshotScale }): Promise<Uint8Array>;
  evaluate<R, A>(fn: (arg: A) => R, arg: A): Promise<Awaited<R>>;
  bringToFront(): Promise<void>;
  on(event: 'framenavigated', listener: (frame: FrameLike) => void): unknown;
  on(event: 'load' | 'close' | 'crash', listener: () => void): unknown;
}

/** What `exposeBinding` hands the callback first. */
export interface BindingSource {
  frame: FrameLike;
}

/** A cookie as Playwright's `addCookies` takes it and `cookies` answers it (RB-040-05). */
export interface CookieLike {
  name: string;
  value: string;
  domain: string;
  path: string;
  /** Seconds since the epoch; -1 for a session cookie. */
  expires: number;
  httpOnly: boolean;
  secure: boolean;
  sameSite: 'Strict' | 'Lax' | 'None';
}

export interface ContextLike {
  newPage(): Promise<PageLike>;
  addInitScript(script: { content: string }): Promise<void>;
  exposeBinding(name: string, callback: (source: BindingSource, payload: unknown) => void): Promise<void>;
  setOffline(offline: boolean): Promise<void>;
  setExtraHTTPHeaders(headers: Record<string, string>): Promise<void>;
  addCookies(cookies: CookieLike[]): Promise<void>;
  cookies(): Promise<CookieLike[]>;
  close(): Promise<void>;
}

/** An emulated position (Playwright's `geolocation` option). */
export interface Geolocation {
  latitude: number;
  longitude: number;
  accuracy: number;
}

/** The options of `browser.newContext` the module sets. */
export interface ContextOptions {
  viewport: { width: number; height: number };
  /** Absent: the display's own ratio. */
  deviceScaleFactor?: number;
  isMobile: boolean;
  hasTouch: boolean;
  userAgent?: string;
  locale?: string;
  timezoneId?: string;
  geolocation?: Geolocation;
  /** Only `geolocation`, granted with an emulated position. */
  permissions?: Array<'geolocation'>;
  colorScheme?: 'light' | 'dark' | 'no-preference';
  reducedMotion?: 'reduce' | 'no-preference';
  forcedColors?: 'active' | 'none';
  contrast?: 'more' | 'no-preference';
  offline?: boolean;
  extraHTTPHeaders?: Record<string, string>;
}

export interface BrowserLike {
  newContext(options: ContextOptions): Promise<ContextLike>;
  close(): Promise<void>;
  version(): string;
  /** The launched browser process (its `pid` owns the view's window, RB-080-39/40); null when
   *  unknown. */
  process?(): { pid?: number | undefined } | null;
  on(event: 'disconnected', listener: () => void): unknown;
}

export interface LaunchOptions {
  headless: false;
  executablePath: string;
}

export interface LauncherLike {
  launch(options: LaunchOptions): Promise<BrowserLike>;
}
