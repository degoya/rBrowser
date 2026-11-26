// The fixture server of the end-to-end run (RB-099-02): tests/fixtures/site on 127.0.0.1 with the
// routes of tests/fixtures/server/post-counter.py - POST /submit counted (K4), GET /count,
// POST /reset, GET /search - plus what the harness needs on top:
//
// - POST /log: a page reports a marker (`{marker, view}`) and the server stamps it in the
//   harness's own clock, so a latency (a file written, the view reloaded) never mixes the clocks
//   of WSL and Windows (K6);
// - a log of every request with the headers asked about (K9: no trust-gated header arrives);
// - mounts of other folders under a prefix, each with an optional headers file
//   (`{"<path below the prefix>": {header: value}}`, the hreflang fixtures' headers.json), and
//   the fixtures' origin (`https://fixture.test`) rewritten to the server's own in every text
//   answer and header (K8);
// - aliases: a file served at another path while asked to (an origin's /robots.txt).
//
// It binds 127.0.0.1 only and keeps nothing on disk. WSL forwards its localhost ports to
// Windows, so the app on Windows reaches it as from scripts/perf.sh.

import { readFileSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import type { IncomingMessage, Server, ServerResponse } from 'node:http';
import type { AddressInfo } from 'node:net';
import { extname, join, resolve, sep } from 'node:path';

const MAX_BODY = 64 * 1024;
/** The request headers the log keeps (lower case). */
const LOGGED_HEADERS = ['x-untrusted', 'user-agent', 'referer', 'content-type'];

const TYPES: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.json': 'application/json',
  '.txt': 'text/plain; charset=utf-8',
  '.xml': 'application/xml',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.webp': 'image/webp',
  '.woff2': 'font/woff2',
  '.webmanifest': 'application/manifest+json',
};
const TEXT = /^(text\/|application\/(json|xml|manifest))/;

export interface Mount {
  /** The URL prefix, with its slashes: `/hreflang-ok/`. */
  prefix: string;
  dir: string;
  /** A JSON file in `dir`: extra response headers by the path below the prefix. */
  headersFile?: string;
}

export interface ServerOptions {
  root: string;
  /** Ports on 127.0.0.1 (0 picks one); the first is the origin. */
  ports: number[];
  mounts?: Mount[];
  /** An origin the fixtures name that answers are rewritten from. */
  rewriteOrigin?: string;
}

export interface Report {
  marker: string;
  view: string;
  at: number;
}

export interface Seen {
  method: string;
  path: string;
  at: number;
  headers: Record<string, string>;
}

export interface FixtureServer {
  /** `http://127.0.0.1:<first port>`. */
  origin: string;
  posts(): number;
  reset(): void;
  reports(): Report[];
  requests(): Seen[];
  /** Serves `file` at `path` from now on; `null` ends that. */
  alias(path: string, file: string | null): void;
  close(): Promise<void>;
}

/** The file under `dir` that `relative` names, or undefined when it would leave `dir`. */
function inside(dir: string, relative: string): string | undefined {
  const base = resolve(dir);
  const file = resolve(base, `.${sep}${relative}`);
  return file === base || file.startsWith(`${base}${sep}`) ? file : undefined;
}

function existing(file: string | undefined): string | undefined {
  if (file === undefined) return undefined;
  try {
    const stat = statSync(file);
    if (stat.isFile()) return file;
    if (stat.isDirectory()) return existing(join(file, 'index.html'));
  } catch {
    // Not there: the caller answers 404.
  }
  return undefined;
}

function readHeaders(mount: Mount): Record<string, Record<string, string>> {
  if (mount.headersFile === undefined) return {};
  const parsed: unknown = JSON.parse(readFileSync(join(mount.dir, mount.headersFile), 'utf8'));
  return typeof parsed === 'object' && parsed !== null ? (parsed as Record<string, Record<string, string>>) : {};
}

async function body(request: IncomingMessage): Promise<string | undefined> {
  const chunks: Buffer[] = [];
  let size = 0;
  for await (const chunk of request) {
    const buffer = chunk as Buffer;
    size += buffer.length;
    if (size > MAX_BODY) return undefined;
    chunks.push(buffer);
  }
  return Buffer.concat(chunks).toString('utf8');
}

function escapeHtml(text: string): string {
  return text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
}

export async function startFixtureServer(options: ServerOptions): Promise<FixtureServer> {
  let posts = 0;
  const reports: Report[] = [];
  const seen: Seen[] = [];
  const aliases = new Map<string, string>();
  const mounts = (options.mounts ?? []).map((mount) => ({ ...mount, headers: readHeaders(mount) }));
  let origin = '';
  const rewrite = (text: string): string =>
    options.rewriteOrigin === undefined ? text : text.replaceAll(options.rewriteOrigin, origin);

  const send = (response: ServerResponse, status: number, type: string, text: string, extra = {}): void => {
    response.writeHead(status, { 'Content-Type': type, 'Cache-Control': 'no-store', ...extra });
    response.end(text);
  };

  const serveFile = (response: ServerResponse, file: string, extra: Record<string, string>): void => {
    const type = TYPES[extname(file).toLowerCase()] ?? 'application/octet-stream';
    const headers: Record<string, string> = { 'Content-Type': type, 'Cache-Control': 'no-store' };
    for (const [name, value] of Object.entries(extra)) headers[name] = rewrite(value);
    const content = readFileSync(file);
    response.writeHead(200, headers);
    response.end(TEXT.test(type) ? rewrite(content.toString('utf8')) : content);
  };

  const handle = async (request: IncomingMessage, response: ServerResponse): Promise<void> => {
    const url = new URL(request.url ?? '/', 'http://127.0.0.1');
    const method = request.method ?? 'GET';
    const headers: Record<string, string> = {};
    for (const name of LOGGED_HEADERS) {
      const value = request.headers[name];
      if (typeof value === 'string') headers[name] = value;
    }
    seen.push({ method, path: url.pathname, at: Date.now(), headers });

    if (method === 'POST') {
      const text = await body(request);
      if (text === undefined) return send(response, 413, 'text/plain', 'too large');
      if (url.pathname === '/submit') {
        posts += 1;
        response.writeHead(303, { Location: `/form.html?posts=${posts}`, 'Content-Length': '0' });
        return void response.end();
      }
      if (url.pathname === '/reset') {
        posts = 0;
        return send(response, 200, 'application/json', JSON.stringify({ posts }));
      }
      if (url.pathname === '/log') {
        const at = Date.now();
        try {
          const entry = JSON.parse(text) as { marker?: unknown; view?: unknown };
          if (typeof entry.marker === 'string') {
            reports.push({ marker: entry.marker, view: typeof entry.view === 'string' ? entry.view : '', at });
          }
        } catch {
          // A report that does not read is dropped.
        }
        response.writeHead(204, { 'Access-Control-Allow-Origin': '*' });
        return void response.end();
      }
      return send(response, 404, 'text/plain', 'not found');
    }
    if (url.pathname === '/count') return send(response, 200, 'application/json', JSON.stringify({ posts }));
    if (url.pathname === '/search') {
      const query = url.searchParams.get('q') ?? '';
      return send(
        response,
        200,
        'text/html; charset=utf-8',
        `<!doctype html><meta charset="utf-8"><title>Search</title><h1 id="result">Results for ${escapeHtml(query)}</h1>`,
      );
    }
    const aliased = aliases.get(url.pathname);
    if (aliased !== undefined) return serveFile(response, aliased, {});
    let path: string;
    try {
      path = decodeURIComponent(url.pathname);
    } catch {
      return send(response, 400, 'text/plain', 'bad path');
    }
    for (const mount of mounts) {
      if (!path.startsWith(mount.prefix)) continue;
      const below = path.slice(mount.prefix.length);
      const file = existing(inside(mount.dir, below));
      if (file === undefined || (mount.headersFile !== undefined && below === mount.headersFile)) break;
      return serveFile(response, file, mount.headers[below] ?? {});
    }
    const file = mounts.some((mount) => path.startsWith(mount.prefix))
      ? undefined
      : existing(inside(options.root, path));
    if (file === undefined) return send(response, 404, 'text/plain', 'not found');
    return serveFile(response, file, {});
  };

  const servers: Server[] = [];
  for (const port of options.ports) {
    const server = createServer((request, response) => {
      handle(request, response).catch(() => {
        if (!response.headersSent) send(response, 500, 'text/plain', 'error');
        else response.end();
      });
    });
    await new Promise<void>((done, fail) => {
      server.once('error', fail);
      server.listen(port, '127.0.0.1', () => done());
    });
    servers.push(server);
  }
  const first = servers[0]?.address() as AddressInfo | null | undefined;
  if (first === null || first === undefined) throw new Error('the fixture server has no port');
  origin = `http://127.0.0.1:${first.port}`;

  return {
    origin,
    posts: () => posts,
    reset: () => {
      posts = 0;
    },
    reports: () => [...reports],
    requests: () => [...seen],
    alias: (path, file) => {
      if (file === null) aliases.delete(path);
      else aliases.set(path, file);
    },
    close: async () => {
      await Promise.all(
        servers.map(
          (server) =>
            new Promise<void>((done) => {
              server.closeAllConnections();
              server.close(() => done());
            }),
        ),
      );
    },
  };
}
