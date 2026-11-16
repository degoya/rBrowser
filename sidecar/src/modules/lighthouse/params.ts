// The parameters of the `lighthouse.*` methods (RB-060-07), read strictly: a caller - the
// `lighthouse` plugin through the `sidecar` capability - names the page and the emulation, never
// a Lighthouse config of its own. A config could load audits and gatherers from any path, so only
// the settings listed here reach Lighthouse, each held to its bounds; anything else is refused.

import { ErrorCode, RpcError } from '../../rpc.ts';

/** The Lighthouse categories a run may ask for (Lighthouse 13 has no PWA category). */
export const CATEGORIES = ['performance', 'accessibility', 'best-practices', 'seo'] as const;
export type Category = (typeof CATEGORIES)[number];

export const FORM_FACTORS = ['mobile', 'desktop'] as const;
/** Lighthouse's throttling methods: simulated, applied in the browser, or none (`provided`). */
export const THROTTLING_METHODS = ['simulate', 'devtools', 'provided'] as const;

/** The longest a poll waits: below `engines.nodeRpcTimeoutMs`'s default of 30 s. */
export const MAX_WAIT_MS = 20_000;
export const DEFAULT_WAIT_MS = 15_000;
const MAX_URL = 8 * 1024;
const MAX_LABEL = 128;
const MAX_USER_AGENT = 512;
const MAX_JOB_ID = 64;

/** The key under which rBrowser names the plugin that calls (`$caller`); the host sets it over
 * anything the plugin sent, so it cannot be forged. Each caller sees and ends only its own jobs
 * (review finding L4 of Charge 7); a call without it is rBrowser's own. */
export const CALLER_KEY = '$caller';
const CALLER = /^[a-z0-9][a-z0-9._-]{0,63}$/;

/** The caller of a call: the plugin id under [`CALLER_KEY`], `''` without one. */
export function readCaller(params: unknown): string {
  if (params === null || typeof params !== 'object' || Array.isArray(params)) {
    return '';
  }
  const caller = (params as Record<string, unknown>)[CALLER_KEY];
  if (caller === undefined) {
    return '';
  }
  if (typeof caller !== 'string' || !CALLER.test(caller)) {
    throw invalid(`${CALLER_KEY} must be a plugin id`);
  }
  return caller;
}
const MAX_SIZE = 10_000;

export interface ScreenEmulation {
  width: number;
  height: number;
  deviceScaleFactor: number;
  mobile: boolean;
  disabled: false;
}

/** The Lighthouse settings of one run, as Lighthouse names them. */
export interface RunSettings {
  formFactor: (typeof FORM_FACTORS)[number];
  screenEmulation: ScreenEmulation;
  /** Null: Lighthouse's own user agent of the form factor. */
  emulatedUserAgent: string | null;
  throttlingMethod: (typeof THROTTLING_METHODS)[number];
  onlyCategories: Category[];
}

export interface StartRequest {
  url: string;
  /** Shown in `lighthouse.status` (the breakpoint). */
  label: string;
  settings: RunSettings;
}

function invalid(message: string): RpcError {
  return new RpcError(ErrorCode.InvalidParams, message);
}

function object(value: unknown, name: string, keys: readonly string[]): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw invalid(`${name} must be an object`);
  }
  const record = value as Record<string, unknown>;
  const unknown = Object.keys(record).find((key) => !keys.includes(key));
  if (unknown !== undefined) {
    throw invalid(`${name}.${unknown} is not accepted`);
  }
  return record;
}

function text(value: unknown, name: string, max: number): string {
  if (typeof value !== 'string' || value.length > max || /[\u0000-\u001f\u007f]/.test(value)) {
    throw invalid(`${name} must be a string of at most ${max} characters without control characters`);
  }
  return value;
}

function oneOf<T extends string>(value: unknown, name: string, allowed: readonly T[]): T {
  if (typeof value !== 'string' || !(allowed as readonly string[]).includes(value)) {
    throw invalid(`${name} must be one of ${allowed.join(', ')}`);
  }
  return value as T;
}

function integer(value: unknown, name: string, min: number, max: number): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < min || value > max) {
    throw invalid(`${name} must be an integer from ${min} to ${max}`);
  }
  return value;
}

/** An `http(s)` URL without credentials. */
export function readUrl(value: unknown): string {
  const raw = text(value, 'url', MAX_URL);
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    throw invalid('url is not a URL');
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    throw invalid('url must be http or https');
  }
  if (url.username !== '' || url.password !== '') {
    throw invalid('url must not carry credentials');
  }
  return url.href;
}

function readEmulation(value: unknown): ScreenEmulation {
  const e = object(value, 'settings.screenEmulation', ['width', 'height', 'deviceScaleFactor', 'mobile', 'disabled']);
  const dpr = e.deviceScaleFactor;
  if (typeof dpr !== 'number' || !Number.isFinite(dpr) || dpr < 0.1 || dpr > 10) {
    throw invalid('settings.screenEmulation.deviceScaleFactor must be a number from 0.1 to 10');
  }
  if (typeof e.mobile !== 'boolean') {
    throw invalid('settings.screenEmulation.mobile must be a boolean');
  }
  if (e.disabled !== undefined && e.disabled !== false) {
    throw invalid('settings.screenEmulation.disabled must be false');
  }
  return {
    width: integer(e.width, 'settings.screenEmulation.width', 1, MAX_SIZE),
    height: integer(e.height, 'settings.screenEmulation.height', 1, MAX_SIZE),
    deviceScaleFactor: dpr,
    mobile: e.mobile,
    disabled: false,
  };
}

function readCategories(value: unknown): Category[] {
  if (value === undefined) {
    return [...CATEGORIES];
  }
  if (!Array.isArray(value) || value.length === 0) {
    throw invalid('settings.onlyCategories must be a list of categories');
  }
  const categories = value.map((entry) => oneOf(entry, 'settings.onlyCategories[]', CATEGORIES));
  return CATEGORIES.filter((category) => categories.includes(category));
}

function readSettings(value: unknown): RunSettings {
  const s = object(value, 'settings', [
    'formFactor',
    'screenEmulation',
    'emulatedUserAgent',
    'throttlingMethod',
    'onlyCategories',
  ]);
  const agent = s.emulatedUserAgent;
  const formFactor = oneOf(s.formFactor, 'settings.formFactor', FORM_FACTORS);
  const screenEmulation = readEmulation(s.screenEmulation);
  // Lighthouse refuses a run whose screen emulation contradicts its form factor.
  if (screenEmulation.mobile !== (formFactor === 'mobile')) {
    throw invalid('settings.screenEmulation.mobile must match settings.formFactor');
  }
  return {
    formFactor,
    screenEmulation,
    emulatedUserAgent:
      agent === undefined || agent === null ? null : text(agent, 'settings.emulatedUserAgent', MAX_USER_AGENT),
    throttlingMethod: oneOf(s.throttlingMethod, 'settings.throttlingMethod', THROTTLING_METHODS),
    onlyCategories: readCategories(s.onlyCategories),
  };
}

/** `lighthouse.start`: `{ url, settings, label? }`. */
export function readStart(params: unknown): StartRequest {
  const p = object(params, 'params', ['url', 'settings', 'label', CALLER_KEY]);
  return {
    url: readUrl(p.url),
    label: p.label === undefined ? '' : text(p.label, 'label', MAX_LABEL),
    settings: readSettings(p.settings),
  };
}

/** The `job` of `lighthouse.poll`, `.report` and `.release`. */
export function readJob(params: unknown): string {
  const p = object(params, 'params', ['job', 'waitMs', 'format', 'offset', CALLER_KEY]);
  if (typeof p.job !== 'string' || !/^[A-Za-z0-9-]+$/.test(p.job) || p.job.length > MAX_JOB_ID) {
    throw invalid('job must be a job id');
  }
  return p.job;
}

/** How long `lighthouse.poll` waits for the job to end, at most [`MAX_WAIT_MS`]. */
export function readWait(params: unknown): number {
  readJob(params);
  const wait = (params as Record<string, unknown>).waitMs;
  if (wait === undefined) {
    return DEFAULT_WAIT_MS;
  }
  if (typeof wait !== 'number' || !Number.isFinite(wait) || wait < 0) {
    throw invalid('waitMs must be a number of milliseconds');
  }
  return Math.min(Math.floor(wait), MAX_WAIT_MS);
}

/** `lighthouse.report`: `{ job, format: "json" | "html", offset? }` - one slice from `offset`. */
export function readReport(params: unknown): { job: string; format: 'json' | 'html'; offset: number } {
  const job = readJob(params);
  const p = params as Record<string, unknown>;
  const offset = p.offset === undefined ? 0 : integer(p.offset, 'offset', 0, Number.MAX_SAFE_INTEGER);
  return { job, format: oneOf(p.format, 'format', ['json', 'html'] as const), offset };
}
