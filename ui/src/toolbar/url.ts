// What the address bar makes of its input. rBrowser is a development browser: no search engine,
// local hosts default to http, everything else to https.

export type UrlInput =
  | { ok: true; url: string }
  | { ok: false; reason: 'empty' | 'unsupported' | 'invalid' };

/** Schemes the address bar passes on as typed. */
const ALLOWED_SCHEMES = new Set(['http:', 'https:', 'file:', 'about:']);
/** Schemes refused outright: script in every view at once, or a page with no origin. */
const REFUSED_SCHEMES = new Set(['javascript:', 'data:', 'blob:', 'vbscript:']);

const SCHEME = /^([a-z][a-z0-9+.-]*):/i;
const HOST_PORT = /^(\[[0-9a-f:]+\]|[a-z0-9.-]+)(:\d{1,5})?([/?#].*)?$/i;
const IPV4 = /^\d{1,3}(\.\d{1,3}){3}$/;
const LOCAL_SUFFIXES = ['.localhost', '.test', '.local', '.internal'];

function isLocalHost(host: string): boolean {
  const name = host.toLowerCase();
  return (
    name === 'localhost' ||
    name.startsWith('[') ||
    IPV4.test(name) ||
    LOCAL_SUFFIXES.some((suffix) => name.endsWith(suffix))
  );
}

function parse(candidate: string): UrlInput {
  try {
    return { ok: true, url: new URL(candidate).href };
  } catch {
    return { ok: false, reason: 'invalid' };
  }
}

export function normalizeUrlInput(input: string): UrlInput {
  const text = input.trim();
  if (text === '') {
    return { ok: false, reason: 'empty' };
  }
  const hostMatch = HOST_PORT.exec(text.replace(/ /g, '%20'));
  const scheme = SCHEME.exec(text)?.[1]?.toLowerCase();
  // `localhost:5173` and `web:3000` look like a scheme; a port number after it says otherwise.
  if (scheme !== undefined && !(hostMatch !== null && hostMatch[2] !== undefined)) {
    const withColon = `${scheme}:`;
    if (REFUSED_SCHEMES.has(withColon)) {
      return { ok: false, reason: 'unsupported' };
    }
    return ALLOWED_SCHEMES.has(withColon) ? parse(text) : { ok: false, reason: 'unsupported' };
  }
  if (hostMatch === null) {
    return { ok: false, reason: 'invalid' };
  }
  const host = hostMatch[1] ?? '';
  const hasPort = hostMatch[2] !== undefined;
  if (isLocalHost(host) || (hasPort && !host.includes('.'))) {
    return parse(`http://${text}`);
  }
  if (!host.includes('.') || host.startsWith('.') || host.endsWith('.')) {
    return { ok: false, reason: 'invalid' };
  }
  return parse(`https://${text}`);
}
