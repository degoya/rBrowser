// The API client panel's model (RB-096-01): the request as the core's model has it
// (crates/rb-api-model, JSON in camelCase), what the core answers, and the panel's small
// decisions - a new request, a body or auth kind switched without losing what was typed, the
// collections by folder, how an answer's body is shown.

import type { Check, Verdict } from './checks';

export const PREFIX = 'plugin.api-client.';
/** The core's failure when a send names a host the project has not allowed yet. */
export const HOST_NOT_ALLOWED = `${PREFIX}errors.host_not_allowed`;

/** The place a failure's text names (RB-096-11, RB-094-21): the status line links there, named by
 * `label` - the private values (`api-client.secrets`) are no setting, their link opens API client ›
 * Variables (RB-094-27). Each key is a literal `setting` object's key for the settings lint
 * (scripts/lib/settings_targets.py). */
const SETTING_LINKS = [
  { code: `${PREFIX}errors.secrets_unreadable`, setting: { key: 'api-client.secrets' }, label: 'panel.openVariables' },
  { code: `${PREFIX}errors.secrets_not_saved`, setting: { key: 'api-client.secrets' }, label: 'panel.openVariables' },
  { code: `${PREFIX}errors.unresolved`, setting: { key: 'api-client.secrets' }, label: 'panel.openVariables' },
  { code: `${PREFIX}errors.too_many_hosts`, setting: { key: 'api-client.allowedHosts' }, label: 'panel.openSetting' },
];

/** The settings key `failure` points at, if any. */
/** The host of a `host_not_allowed` failure (RB-096-13): a send, a collection run or a benchmark
 * to a host the project has not allowed; `null` for any other failure. */
export function blockedHost(failure: unknown): string | null {
  if (!(failure instanceof Error) || failure.message !== HOST_NOT_ALLOWED) return null;
  const host = (failure as Error & { params?: Record<string, unknown> }).params?.host;
  return typeof host === 'string' && host !== '' ? host : null;
}

/** The place `failure`'s text names - its key and the i18n key of the link's label -, if any. */
export function linkOf(failure: unknown): { key: string; label: string } | undefined {
  const link = failure instanceof Error ? SETTING_LINKS.find((one) => one.code === failure.message) : undefined;
  return link ? { key: link.setting.key, label: link.label } : undefined;
}

export interface Entry {
  name: string;
  value: string;
  enabled: boolean;
}

export interface Part {
  name: string;
  value: string;
  file?: string;
  contentType?: string;
}

export type Body =
  | { kind: 'none' }
  | { kind: 'json'; text: string }
  | { kind: 'form'; fields: Entry[] }
  | { kind: 'multipart'; parts: Part[] }
  | { kind: 'raw'; text: string }
  | { kind: 'binary'; path: string }
  | { kind: 'graphql'; query: string; variables: string };

export type BodyKind = Body['kind'];

export type Auth =
  | { type: 'none' }
  | { type: 'basic'; username: string; password: string }
  | { type: 'bearer'; token: string }
  | { type: 'apiKey'; name: string; value: string; in: 'header' | 'query' }
  | { type: 'digest'; username: string; password: string }
  | {
      type: 'oauth2';
      grant: 'clientCredentials' | 'authorizationCode';
      tokenUrl: string;
      authUrl?: string;
      clientId: string;
      clientSecret?: string;
      scope?: string;
      redirectUri?: string;
    };

export type AuthType = Auth['type'];

export interface Options {
  timeoutMs: number;
  followRedirects: boolean;
  verifyTls: boolean;
}

export interface ApiRequest {
  name: string;
  method: string;
  url: string;
  headers: Entry[];
  body: Body;
  auth: Auth;
  options: Options;
  comments?: string[];
  /** RB-096-04: what the answer must show. */
  checks?: Check[];
}

export interface Listed {
  id: string;
  name: string;
  method: string;
  url: string;
  problem?: string;
}

export interface State {
  environment: string | null;
  environments: string[];
  variables: { name: string; secret: boolean }[];
  requests: Listed[];
  /** RB-096-11: every folder below .rbrowser/api, an empty one too. */
  folders?: string[];
  allowedHosts: string[];
  envProblem?: string | null;
  /** RB-096-08: a request the network panel opened here, carried once (`captured.ts`). */
  draft?: unknown;
  /** RB-094-27: the side tab a link asked for (`sideTabs.ts`), carried once. */
  tab?: unknown;
}

export interface Timings {
  dns: number | null;
  connect: number | null;
  tls: number | null;
  firstByte: number;
  total: number;
}

export interface Answer {
  method: string;
  url: string;
  status: number;
  statusText: string;
  headers: [string, string][];
  contentType: string | null;
  body: string;
  bodyEncoding: 'text' | 'base64';
  size: number;
  truncated: boolean;
  timings: Timings;
  redirects: string[];
  environment: string | null;
  /** RB-096-04: the verdicts of the request's checks, when it has any. */
  checks?: Verdict[];
}

export interface HistoryEntry {
  at: number;
  id?: string;
  method: string;
  url: string;
  environment?: string;
  status?: number;
  totalMs?: number;
  error?: string;
}

export const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const;
export const BODY_KINDS: readonly BodyKind[] = ['none', 'json', 'form', 'multipart', 'raw', 'binary', 'graphql'];
export const AUTH_TYPES: readonly AuthType[] = ['none', 'basic', 'bearer', 'apiKey', 'digest', 'oauth2'];
/** Kept and stored, not sent yet: the core answers `auth_not_supported`. */
export const AUTH_NOT_SENT: readonly AuthType[] = ['digest', 'oauth2'];

/** A new request: `GET {{baseUrl}}/`. */
export function emptyRequest(): ApiRequest {
  return {
    name: '',
    method: 'GET',
    url: '{{baseUrl}}/',
    headers: [],
    body: { kind: 'none' },
    auth: { type: 'none' },
    options: { timeoutMs: 30000, followRedirects: true, verifyTls: true },
  };
}

/** The text a body holds, whatever its kind. */
function textOf(body: Body): string {
  if (body.kind === 'json' || body.kind === 'raw') return body.text;
  if (body.kind === 'graphql') return body.query;
  return '';
}

/** `body` as kind `kind`, its text kept where both hold one. */
export function withBodyKind(body: Body, kind: BodyKind): Body {
  if (body.kind === kind) return body;
  const text = textOf(body);
  switch (kind) {
    case 'none':
      return { kind };
    case 'json':
    case 'raw':
      return { kind, text };
    case 'graphql':
      return { kind, query: text, variables: '' };
    case 'form':
      return { kind, fields: [] };
    case 'multipart':
      return { kind, parts: [] };
    case 'binary':
      return { kind, path: '' };
  }
}

/** `auth` as type `type`, the user name and secret kept where both have them. */
export function withAuthType(auth: Auth, type: AuthType): Auth {
  if (auth.type === type) return auth;
  const username = auth.type === 'basic' || auth.type === 'digest' ? auth.username : '';
  const password = auth.type === 'basic' || auth.type === 'digest' ? auth.password : '';
  switch (type) {
    case 'none':
      return { type };
    case 'basic':
    case 'digest':
      return { type, username, password };
    case 'bearer':
      return { type, token: auth.type === 'bearer' ? auth.token : '' };
    case 'apiKey':
      return { type, name: 'X-Api-Key', value: '', in: 'header' };
    case 'oauth2':
      return { type, grant: 'clientCredentials', tokenUrl: '', clientId: '' };
  }
}

export interface Folder {
  folder: string;
  requests: Listed[];
}

/** The folder of request `id` (`""` for the top). */
export function folderOf(id: string): string {
  const at = id.lastIndexOf('/');
  return at < 0 ? '' : id.slice(0, at);
}

/** `requests` matching `query` (title, URL or id), by folder, the top first. */
export function byFolder(requests: readonly Listed[], query = ''): Folder[] {
  const needle = query.trim().toLowerCase();
  const shown = needle
    ? requests.filter((r) => [r.name, r.url, r.id, r.method].some((text) => text.toLowerCase().includes(needle)))
    : requests;
  const folders = new Map<string, Listed[]>();
  for (const request of shown) {
    const folder = folderOf(request.id);
    folders.set(folder, [...(folders.get(folder) ?? []), request]);
  }
  return [...folders.entries()]
    .sort(([a], [b]) => (a === '' ? -1 : b === '' ? 1 : a.localeCompare(b)))
    .map(([folder, list]) => ({ folder, requests: list }));
}

export type BodyView = 'json' | 'xml' | 'html' | 'image' | 'text' | 'binary';

/** How an answer's body is shown, by its content type and encoding. */
export function bodyView(answer: Pick<Answer, 'contentType' | 'bodyEncoding'>): BodyView {
  const type = (answer.contentType ?? '').toLowerCase();
  if (type.startsWith('image/')) return 'image';
  if (answer.bodyEncoding === 'base64') return 'binary';
  if (type.includes('json')) return 'json';
  if (type.includes('html')) return 'html';
  if (type.includes('xml')) return 'xml';
  return 'text';
}

/** The body as JSON, or undefined when it does not parse. */
export function parsedBody(answer: Pick<Answer, 'body' | 'bodyEncoding'>): unknown {
  if (answer.bodyEncoding !== 'text') return undefined;
  try {
    return JSON.parse(answer.body) as unknown;
  } catch {
    return undefined;
  }
}

/** A size as people read it: `512 B`, `1.5 KiB`, `2.0 MiB`. */
export function bytes(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KiB`;
  return `${(size / 1024 / 1024).toFixed(1)} MiB`;
}

/** A time in milliseconds, one decimal under 10 ms. */
export function millis(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return '-';
  return ms < 10 ? `${ms.toFixed(1)} ms` : `${Math.round(ms)} ms`;
}

/** The status's colour: success, a redirect, the client's or the server's error. */
export function statusTone(status: number): 'success' | 'info' | 'warning' | 'error' {
  if (status >= 500) return 'error';
  if (status >= 400) return 'warning';
  if (status >= 300) return 'info';
  return 'success';
}
