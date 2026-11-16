// The parameters of the `webkit.*` methods, checked before anything reaches Playwright
// (RB-040-04). rBrowser is the only peer on stdio, but the values come from a project config and
// from pages, so every field is checked for type, range and form; anything else is refused with
// `InvalidParams`.

import { ErrorCode, RpcError } from '../rpc.ts';
import type { ContextOptions, Geolocation, MediaOptions, ScreenshotScale } from './types.ts';

/** The schemes a WebKit view may load. */
const SCHEMES = new Set(['http:', 'https:', 'file:', 'about:']);
/** The longest URL accepted. */
const MAX_URL = 32 * 1024;
/** The longest user agent, locale or time zone. */
const MAX_TEXT = 1024;
/** The largest pixel ratio (rb-config's limit). */
const MAX_DPR = 10;
/** The largest accuracy of an emulated position, in metres. */
const MAX_ACCURACY = 1_000_000;
/** The most extra request headers. */
const MAX_HEADERS = 64;
/** A header name (RFC 9110 token). */
const HEADER_NAME = /^[!#$%&'*+.^_`|~0-9A-Za-z-]{1,256}$/;

/** A view's options: the context's and the media type. */
export interface ViewOptions extends ContextOptions {
  media?: 'screen' | 'print';
}

export function invalid(message: string): RpcError {
  return new RpcError(ErrorCode.InvalidParams, message);
}

export function object(value: unknown, what: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw invalid(`${what} must be an object`);
  }
  return value as Record<string, unknown>;
}

export function integer(value: unknown, what: string, min: number, max: number): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < min || value > max) {
    throw invalid(`${what} must be an integer from ${min} to ${max}`);
  }
  return value;
}

export function finite(value: unknown, what: string, min: number, max: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < min || value > max) {
    throw invalid(`${what} must be a number from ${min} to ${max}`);
  }
  return value;
}

export function bool(value: unknown, what: string): boolean {
  if (typeof value !== 'boolean') {
    throw invalid(`${what} must be true or false`);
  }
  return value;
}

export function text(value: unknown, what: string, max = MAX_TEXT): string {
  if (typeof value !== 'string' || value.length > max) {
    throw invalid(`${what} must be a string of at most ${max} characters`);
  }
  return value;
}

function oneOf<T extends string>(value: unknown, what: string, allowed: readonly T[]): T {
  if (!(allowed as readonly unknown[]).includes(value)) {
    throw invalid(`${what} must be one of ${allowed.join(', ')}`);
  }
  return value as T;
}

/** A view id: rBrowser's `ViewId`, a non-negative integer. */
export function viewId(params: Record<string, unknown>): number {
  return integer(params.view, 'view', 0, 0xffff_ffff);
}

/** A URL a view may load: http(s), file or about, absolute. */
export function url(value: unknown): string {
  const raw = text(value, 'url', MAX_URL);
  let parsed: URL;
  try {
    parsed = new URL(raw);
  } catch {
    throw invalid('url must be an absolute URL');
  }
  if (!SCHEMES.has(parsed.protocol)) {
    throw invalid(`url scheme ${parsed.protocol} is not allowed`);
  }
  return raw;
}

function headers(value: unknown): Record<string, string> {
  const entries = Object.entries(object(value, 'extraHTTPHeaders'));
  if (entries.length > MAX_HEADERS) {
    throw invalid(`extraHTTPHeaders may hold at most ${MAX_HEADERS} headers`);
  }
  const out: Record<string, string> = {};
  for (const [name, raw] of entries) {
    const header = text(raw, `header ${name}`, 8192);
    if (!HEADER_NAME.test(name) || /[\r\n\0]/.test(header)) {
      throw invalid('extraHTTPHeaders holds an invalid header');
    }
    out[name] = header;
  }
  return out;
}

/** A pixel ratio above 0, at most `MAX_DPR`. */
function pixelRatio(value: unknown): number {
  const ratio = finite(value, 'deviceScaleFactor', 0, MAX_DPR);
  if (ratio === 0) throw invalid(`deviceScaleFactor must be above 0 and at most ${MAX_DPR}`);
  return ratio;
}

function geolocation(value: unknown): Geolocation {
  const o = object(value, 'geolocation');
  return {
    latitude: finite(o.latitude, 'geolocation.latitude', -90, 90),
    longitude: finite(o.longitude, 'geolocation.longitude', -180, 180),
    accuracy: finite(o.accuracy, 'geolocation.accuracy', 0, MAX_ACCURACY),
  };
}

function permissions(value: unknown): Array<'geolocation'> {
  if (!Array.isArray(value)) throw invalid('permissions must be a list');
  return value.map((entry) => oneOf(entry, 'permissions', ['geolocation'] as const));
}

/** A screenshot's scale: `css` (one pixel per CSS px) or `device`. */
export function screenshotScale(value: unknown): ScreenshotScale {
  return oneOf(value, 'scale', ['css', 'device'] as const);
}

/** The view options rBrowser sends (`rb_engine::Emulation` mapped by the engine). */
export function viewOptions(value: unknown): ViewOptions {
  const o = object(value, 'options');
  const viewport = object(o.viewport, 'options.viewport');
  const options: ViewOptions = {
    viewport: {
      width: integer(viewport.width, 'viewport.width', 1, 10_000),
      height: integer(viewport.height, 'viewport.height', 1, 10_000),
    },
    isMobile: bool(o.isMobile, 'isMobile'),
    hasTouch: bool(o.hasTouch, 'hasTouch'),
  };
  if (o.deviceScaleFactor !== undefined) options.deviceScaleFactor = pixelRatio(o.deviceScaleFactor);
  if (o.userAgent !== undefined) options.userAgent = text(o.userAgent, 'userAgent');
  if (o.locale !== undefined) options.locale = text(o.locale, 'locale', 64);
  if (o.timezoneId !== undefined) options.timezoneId = text(o.timezoneId, 'timezoneId', 64);
  if (o.geolocation !== undefined) options.geolocation = geolocation(o.geolocation);
  if (o.permissions !== undefined) options.permissions = permissions(o.permissions);
  if (o.colorScheme !== undefined) {
    options.colorScheme = oneOf(o.colorScheme, 'colorScheme', ['light', 'dark', 'no-preference'] as const);
  }
  if (o.reducedMotion !== undefined) {
    options.reducedMotion = oneOf(o.reducedMotion, 'reducedMotion', ['reduce', 'no-preference'] as const);
  }
  if (o.forcedColors !== undefined) {
    options.forcedColors = oneOf(o.forcedColors, 'forcedColors', ['active', 'none'] as const);
  }
  if (o.contrast !== undefined) {
    options.contrast = oneOf(o.contrast, 'contrast', ['more', 'no-preference'] as const);
  }
  if (o.offline !== undefined) options.offline = bool(o.offline, 'offline');
  if (o.extraHTTPHeaders !== undefined) options.extraHTTPHeaders = headers(o.extraHTTPHeaders);
  if (o.media !== undefined) options.media = oneOf(o.media, 'media', ['screen', 'print'] as const);
  return options;
}

/** The options only a new context can change. */
const CONTEXT_BOUND = ['deviceScaleFactor', 'isMobile', 'hasTouch', 'userAgent', 'locale', 'timezoneId'] as const;

/** Whether going from `before` to `after` needs a new context (WebKit cannot change these live). */
export function needsNewContext(before: ViewOptions, after: ViewOptions): boolean {
  const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
  return (
    CONTEXT_BOUND.some((key) => before[key] !== after[key]) ||
    !same(before.geolocation, after.geolocation) ||
    !same(before.permissions, after.permissions)
  );
}

/** The context options of `options` (without the media type, which is the page's). */
export function contextOptions(options: ViewOptions): ContextOptions {
  const { media: _media, ...context } = options;
  return context;
}

/** What `page.emulateMedia` gets for `options`. */
export function mediaOptions(options: ViewOptions): MediaOptions {
  return {
    colorScheme: options.colorScheme ?? null,
    reducedMotion: options.reducedMotion ?? null,
    forcedColors: options.forcedColors ?? null,
    contrast: options.contrast ?? null,
    media: options.media ?? null,
  };
}
