import { createHash } from 'node:crypto';
import { mkdtemp, readFile, readdir, readlink, rm, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { parseProgress } from '../modules/playwright.ts';
import { ErrorCode } from '../rpc.ts';
import { type DownloadResponse, type InstallDeps, installWebkit, progressLine } from './install.ts';
import { MIRRORS, PIN_MARKER, type Pin } from './pin.ts';
import { makeZip } from './zip_fixture.ts';

const ARCHIVE = makeZip([
  { name: 'Playwright.exe', data: 'MZ', deflate: true },
  { name: 'WebKit2.dll', data: 'dll' },
]);
const TAMPERED = makeZip([{ name: 'Playwright.exe', data: 'MZ evil' }]);

function pinOf(archive: Buffer): Pin {
  return {
    playwright: '1.65.0',
    revision: '2388',
    archive: 'webkit-win64.zip',
    sha256: createHash('sha256').update(archive).digest('hex'),
    bytes: archive.length,
  };
}

function response(body: Buffer, url: string, status = 200): DownloadResponse {
  return {
    ok: status === 200,
    status,
    url,
    headers: { get: (name) => (name === 'content-length' ? String(body.length) : null) },
    body: (async function* () {
      yield body.subarray(0, 10);
      yield body.subarray(10);
    })(),
  };
}

/** Deps whose mirrors answer `bodies` in order (an Error is a network failure). */
function deps(bodies: Array<Buffer | Error | { body: Buffer; url: string }>, pin = pinOf(ARCHIVE)) {
  const asked: string[] = [];
  const d: InstallDeps & { pin: Pin } = {
    fetch: async (url) => {
      asked.push(url);
      const next = bodies[asked.length - 1];
      if (next === undefined || next instanceof Error) throw next ?? new Error('no answer');
      if (Buffer.isBuffer(next)) return response(next, url);
      return response(next.body, next.url);
    },
    platform: 'win32',
    pin,
  };
  return { d, asked };
}

let root = '';
beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'rb-pw-'));
});
afterEach(async () => {
  await rm(root, { recursive: true, force: true });
});

const signal = () => new AbortController().signal;

describe('installWebkit', () => {
  it('downloads the pinned archive, verifies it, unpacks it and writes the marker', async () => {
    const { d, asked } = deps([ARCHIVE]);
    const lines: string[] = [];
    await installWebkit(d, root, '2388', signal(), (line) => lines.push(line));
    expect(asked).toEqual([`${MIRRORS[0]}/builds/webkit/2388/webkit-win64.zip`]);
    const build = join(root, 'webkit-2388');
    expect((await readdir(build)).sort()).toEqual(['INSTALLATION_COMPLETE', 'Playwright.exe', PIN_MARKER, 'WebKit2.dll']);
    expect(await readFile(join(build, PIN_MARKER), 'utf8')).toBe(d.pin.sha256);
    expect(await readFile(join(build, 'Playwright.exe'), 'utf8')).toBe('MZ');
    expect(lines.map(parseProgress).at(-1)).toBe(100);
    expect(await readdir(root)).toEqual(['webkit-2388']);
  });

  it('refuses an archive whose SHA-256 differs and installs nothing', async () => {
    const { d, asked } = deps([TAMPERED, TAMPERED]);
    await expect(installWebkit(d, root, '2388', signal(), () => {})).rejects.toMatchObject({
      code: ErrorCode.ChecksumMismatch,
    });
    expect(asked).toHaveLength(2);
    expect(await readdir(root)).toEqual([]);
  });

  it('takes the next mirror after a tampered or failed one', async () => {
    const { d, asked } = deps([new Error('ECONNRESET'), ARCHIVE]);
    await installWebkit(d, root, '2388', signal(), () => {});
    expect(asked[1]).toBe(`${MIRRORS[1]}/builds/webkit/2388/webkit-win64.zip`);
    const { d: d2 } = deps([TAMPERED, ARCHIVE]);
    await rm(join(root, 'webkit-2388'), { recursive: true });
    await installWebkit(d2, root, '2388', signal(), () => {});
    expect(await readFile(join(root, 'webkit-2388', 'INSTALLATION_COMPLETE'), 'utf8')).toBe('');
  });

  it('refuses a download redirected away from HTTPS', async () => {
    const { d } = deps([
      { body: ARCHIVE, url: 'http://mirror.test/webkit-win64.zip' },
      { body: ARCHIVE, url: 'http://mirror.test/webkit-win64.zip' },
    ]);
    await expect(installWebkit(d, root, '2388', signal(), () => {})).rejects.toMatchObject({ code: ErrorCode.InstallFailed });
  });

  it('refuses an archive larger than the pin', async () => {
    const big = Buffer.concat([ARCHIVE, Buffer.alloc(10)]);
    const { d } = deps([big, big]);
    await expect(installWebkit(d, root, '2388', signal(), () => {})).rejects.toMatchObject({ code: ErrorCode.InstallFailed });
  });

  it('refuses a revision the pin is not for, and another platform', async () => {
    const { d } = deps([ARCHIVE]);
    await expect(installWebkit(d, root, '2400', signal(), () => {})).rejects.toThrow('pin the new archive');
    await expect(installWebkit({ ...d, platform: 'linux', pin: null }, root, '2388', signal(), () => {})).rejects.toThrow(
      'Windows x64 and macOS 14 or newer',
    );
  });

  it('installs a macOS build with its executables and framework links (RB-097-02)', async () => {
    const archive = makeZip([
      { name: 'pw_run.sh', data: '#!/bin/sh', mode: 0o100755 },
      { name: 'WebKit.framework/Versions/A/WebKit', data: 'dylib', mode: 0o100644, deflate: true },
      { name: 'WebKit.framework/Versions/Current', data: 'A', mode: 0o120755 },
      { name: 'WebKit.framework/WebKit', data: 'Versions/Current/WebKit', mode: 0o120755 },
    ]);
    const pin = { ...pinOf(archive), revision: '2251', archive: 'webkit-mac-14-arm64.zip' };
    const { d, asked } = deps([archive], pin);
    await installWebkit({ ...d, platform: 'darwin' }, root, '2251', signal(), () => {});
    expect(asked).toEqual([`${MIRRORS[0]}/builds/webkit/2251/webkit-mac-14-arm64.zip`]);
    const build = join(root, 'webkit-2251');
    expect((await stat(join(build, 'pw_run.sh'))).mode & 0o100).toBe(0o100);
    expect(await readlink(join(build, 'WebKit.framework', 'Versions', 'Current'))).toBe('A');
    expect(await readFile(join(build, 'WebKit.framework', 'WebKit'), 'utf8')).toBe('dylib');
    expect(await readFile(join(build, PIN_MARKER), 'utf8')).toBe(pin.sha256);
  });

  it('replaces a half-written build folder', async () => {
    const { d } = deps([ARCHIVE]);
    const { mkdir, writeFile } = await import('node:fs/promises');
    await mkdir(join(root, 'webkit-2388'));
    await writeFile(join(root, 'webkit-2388', 'stale.dll'), 'old');
    await installWebkit(d, root, '2388', signal(), () => {});
    expect(await readdir(join(root, 'webkit-2388'))).not.toContain('stale.dll');
  });
});

describe('progressLine', () => {
  it('reads like Playwright, so parseProgress takes it', () => {
    expect(parseProgress(progressLine(42, 10 * 1024 * 1024))).toBe(42);
  });
});
