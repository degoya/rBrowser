// The WebKit download of `engines.update` (RB-040-04, threat model T11): the pinned archive from
// Playwright's mirrors over HTTPS, checked against our SHA-256 before anything is unpacked,
// unpacked into a temporary folder beside the build's, moved into place and marked with
// Playwright's `INSTALLATION_COMPLETE` - the layout playwright-core's launcher expects - and, last,
// rBrowser's `RBROWSER_PIN` naming the archive's SHA-256, the only marker rBrowser trusts.
// Playwright's own installer, which verifies nothing, is never run.

import { createHash, randomBytes } from 'node:crypto';
import { mkdir, rename, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import { ErrorCode, RpcError } from '../rpc.ts';
import { MARKER, PIN_MARKER, type Pin, WEBKIT_PIN, archiveUrls, buildDirName } from './pin.ts';
import { extractAll } from './zip.ts';

/** The part of a fetch `Response` the download reads. */
export interface DownloadResponse {
  ok: boolean;
  status: number;
  /** The final URL after redirects. */
  url: string;
  headers: { get(name: string): string | null };
  body: AsyncIterable<Uint8Array> | null;
}

export interface InstallDeps {
  fetch(url: string, signal: AbortSignal): Promise<DownloadResponse>;
  platform: NodeJS.Platform;
  arch: string;
  /** The archive to install ([`WEBKIT_PIN`]; tests pin archives of their own). */
  pin: Pin;
}

/** What a download got: the bytes and their SHA-256. */
interface Downloaded {
  data: Buffer;
  sha256: string;
}

/** A progress line in the form Playwright's CLI prints (`parseProgress` reads it). */
export function progressLine(percent: number, bytes: number = WEBKIT_PIN.bytes): string {
  const mib = (bytes / (1024 * 1024)).toFixed(1);
  return `| ${Math.min(100, Math.max(0, Math.floor(percent)))}% of ${mib} MiB`;
}

async function download(
  deps: InstallDeps,
  url: string,
  signal: AbortSignal,
  onLine: (line: string) => void,
): Promise<Downloaded> {
  const response = await deps.fetch(url, signal);
  if (!response.ok || response.body === null) {
    throw new Error(`HTTP ${response.status}`);
  }
  if (!response.url.startsWith('https://')) {
    throw new Error('the download left HTTPS');
  }
  const pin = deps.pin;
  const declared = Number(response.headers.get('content-length') ?? '');
  const expected = Number.isFinite(declared) && declared > 0 ? declared : pin.bytes;
  if (expected > pin.bytes) throw new Error(`the archive is ${expected} bytes, the pin says ${pin.bytes}`);
  const hash = createHash('sha256');
  const chunks: Buffer[] = [];
  let received = 0;
  let lastPercent = -1;
  for await (const chunk of response.body) {
    received += chunk.byteLength;
    if (received > pin.bytes) throw new Error(`the archive is larger than the pinned ${pin.bytes} bytes`);
    hash.update(chunk);
    chunks.push(Buffer.from(chunk));
    const percent = Math.floor((received / expected) * 100);
    if (percent !== lastPercent && percent < 100) {
      lastPercent = percent;
      onLine(progressLine(percent, pin.bytes));
    }
  }
  return { data: Buffer.concat(chunks, received), sha256: hash.digest('hex') };
}

/** Downloads the pinned archive from the first mirror that delivers it with the pinned SHA-256. */
export async function fetchVerified(deps: InstallDeps, signal: AbortSignal, onLine: (line: string) => void): Promise<Buffer> {
  let mismatch = false;
  let lastError = 'no mirror';
  for (const url of archiveUrls(deps.pin)) {
    signal.throwIfAborted();
    try {
      const got = await download(deps, url, signal, onLine);
      if (got.sha256 === deps.pin.sha256) return got.data;
      mismatch = true;
      lastError = `SHA-256 ${got.sha256} does not match the pinned ${deps.pin.sha256}`;
      process.stderr.write(`webkit download from ${url}: ${lastError}\n`);
    } catch (error) {
      if (signal.aborted) throw error;
      lastError = error instanceof Error ? error.message : String(error);
      process.stderr.write(`webkit download from ${url} failed: ${lastError}\n`);
    }
  }
  if (mismatch) {
    throw new RpcError(ErrorCode.ChecksumMismatch, `the WebKit archive does not match its pinned SHA-256; nothing was installed (${lastError})`);
  }
  throw new RpcError(ErrorCode.InstallFailed, `the WebKit archive could not be downloaded: ${lastError}`);
}

/** Installs the pinned WebKit build into `browsersPath` for the `revision` playwright-core wants. */
export async function installWebkit(
  deps: InstallDeps,
  browsersPath: string,
  revision: string,
  signal: AbortSignal,
  onLine: (line: string) => void,
): Promise<void> {
  if (deps.platform !== 'win32' || deps.arch !== 'x64') {
    throw new RpcError(ErrorCode.InstallFailed, 'the pinned WebKit build is for Windows x64 only');
  }
  if (revision !== deps.pin.revision) {
    throw new RpcError(
      ErrorCode.InstallFailed,
      `playwright-core wants WebKit ${revision}, the pin is ${deps.pin.revision}: pin the new archive (sidecar/src/webkit/pin.ts)`,
    );
  }
  const archive = await fetchVerified(deps, signal, onLine);
  await mkdir(browsersPath, { recursive: true });
  const staging = join(browsersPath, `.rb-webkit-${randomBytes(8).toString('hex')}`);
  const target = join(browsersPath, buildDirName(revision));
  try {
    await mkdir(staging);
    await extractAll(archive, staging, signal);
    await rm(target, { recursive: true, force: true });
    await rename(staging, target);
    await writeFile(join(target, MARKER), '');
    // Last: only now is the build the pinned one.
    await writeFile(join(target, PIN_MARKER), deps.pin.sha256);
    onLine(progressLine(100, deps.pin.bytes));
  } finally {
    await rm(staging, { recursive: true, force: true });
  }
}

/** The real dependencies: Node's fetch, following redirects. */
export function realInstallDeps(): InstallDeps {
  return {
    fetch: async (url, signal) => {
      const response = await fetch(url, { signal, redirect: 'follow' });
      return {
        ok: response.ok,
        status: response.status,
        url: response.url,
        headers: response.headers,
        body: response.body as AsyncIterable<Uint8Array> | null,
      };
    },
    platform: process.platform,
    arch: process.arch,
    pin: WEBKIT_PIN,
  };
}
