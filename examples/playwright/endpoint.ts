// Where rBrowser's CDP proxy is and how to prove the token (RB-040-06). Read from the
// environment, never written into a script:
//
// - RBROWSER_CDP_URL: the proxy's URL - `url` of the MCP tool `cdp_endpoint`
//   (http://127.0.0.1:<port>), its `urlWithToken` (the token in the path) or a ws:// URL;
// - RBROWSER_CDP_TOKEN: the token, sent as `Authorization: Bearer <token>` (not needed with the
//   path form);
// - RBROWSER_CDP_ENDPOINT: instead of both, a JSON file holding the answer of `cdp_endpoint`.
//   Inside WSL (WSL_DISTRO_NAME is set) its `wsl` route is taken: the same URL in mirrored mode,
//   the `rbrowser cdp-tunnel` port in NAT mode.
//
// No error message here carries the token.

/** The proxy URL for `chromium.connectOverCDP(url, { headers })` and the headers to send. */
export interface Endpoint {
  url: string;
  headers: Record<string, string>;
}

/** The environment variables this module reads. */
export type Env = Readonly<Record<string, string | undefined>>;

const HELP =
  'set RBROWSER_CDP_URL (and RBROWSER_CDP_TOKEN), or RBROWSER_CDP_ENDPOINT to a file with the answer of the MCP tool cdp_endpoint';

function text(value: unknown): string | undefined {
  return typeof value === 'string' && value.length > 0 ? value : undefined;
}

function record(value: unknown): Record<string, unknown> | undefined {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

/** The headers of a `cdp_endpoint` answer, only string values. */
function headersOf(value: unknown): Record<string, string> {
  const headers: Record<string, string> = {};
  for (const [name, header] of Object.entries(record(value) ?? {})) {
    if (typeof header === 'string') {
      headers[name] = header;
    }
  }
  return headers;
}

/**
 * The endpoint of a `cdp_endpoint` answer. `inWsl` takes the route from WSL: its URL when it
 * has one, its error (NAT mode without the tunnel) as the failure.
 */
export function endpointFromAnswer(answer: unknown, inWsl: boolean): Endpoint {
  const fields = record(answer);
  if (fields === undefined) {
    throw new Error('the cdp_endpoint answer is no JSON object');
  }
  if (fields.enabled === false || text(fields.url) === undefined) {
    throw new Error('the cdp_endpoint answer has no URL: switch the proxy on in Settings > Automation > CDP proxy');
  }
  const headers = headersOf(fields.headers);
  let url = text(fields.url) as string;
  const wsl = record(fields.wsl);
  if (inWsl && wsl !== undefined) {
    const error = text(wsl.error);
    if (error !== undefined) {
      const hint = text(wsl.hint);
      throw new Error(`no route from WSL: ${error}${hint === undefined ? '' : ` (${hint})`}`);
    }
    url = text(wsl.url) ?? url;
  }
  return { url, headers };
}

/** The endpoint from `env`; `readFile` reads the file RBROWSER_CDP_ENDPOINT names. */
export function resolveEndpoint(env: Env, readFile: (path: string) => string): Endpoint {
  const url = text(env.RBROWSER_CDP_URL);
  if (url !== undefined) {
    const token = text(env.RBROWSER_CDP_TOKEN);
    return { url, headers: token === undefined ? {} : { Authorization: `Bearer ${token}` } };
  }
  const file = text(env.RBROWSER_CDP_ENDPOINT);
  if (file === undefined) {
    throw new Error(`no rBrowser CDP endpoint: ${HELP}`);
  }
  let answer: unknown;
  try {
    answer = JSON.parse(readFile(file));
  } catch {
    throw new Error(`RBROWSER_CDP_ENDPOINT is no readable JSON file: ${file}`);
  }
  return endpointFromAnswer(answer, text(env.WSL_DISTRO_NAME) !== undefined);
}
