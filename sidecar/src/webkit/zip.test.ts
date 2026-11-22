import { lstat, mkdtemp, readFile, readdir, readlink, rm, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { ZipError, extractAll, readEntries, safeName } from './zip.ts';
import { makeZip } from './zip_fixture.ts';

let dir = '';
beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), 'rb-zip-'));
});
afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

describe('extractAll', () => {
  it('unpacks stored and deflated files and folders', async () => {
    const zip = makeZip([
      { name: 'Playwright.exe', data: 'MZ binary', deflate: true },
      { name: 'lib/' },
      { name: 'lib/a.dll', data: 'dll' },
    ]);
    expect(await extractAll(zip, dir)).toBe(3);
    expect(await readFile(join(dir, 'Playwright.exe'), 'utf8')).toBe('MZ binary');
    expect(await readFile(join(dir, 'lib', 'a.dll'), 'utf8')).toBe('dll');
  });

  it('refuses an entry whose CRC-32 is wrong and leaves its file unwritten', async () => {
    const zip = makeZip([{ name: 'a.txt', data: 'x', crc: 1 }]);
    await expect(extractAll(zip, dir)).rejects.toBeInstanceOf(ZipError);
    expect(await readdir(dir)).toEqual([]);
  });

  it('refuses encrypted entries', () => {
    expect(() => readEntries(makeZip([{ name: 'a', data: 'x', flags: 1 }]))).toThrow('encrypted');
  });

  it('refuses names that leave the folder', () => {
    for (const name of ['../evil.dll', 'lib/../../evil', '/abs', 'C:/Windows/evil', '..\\evil', 'a/./b']) {
      expect(() => readEntries(makeZip([{ name, data: 'x' }])), name).toThrow('unsafe entry name');
    }
  });

  it('refuses a file twice', async () => {
    const zip = makeZip([
      { name: 'a', data: '1' },
      { name: 'a', data: '2' },
    ]);
    await expect(extractAll(zip, dir)).rejects.toThrow();
  });

  it('refuses what is no archive', () => {
    expect(() => readEntries(Buffer.from('not a zip at all, but long enough'))).toThrow('no end of central directory');
  });
});

describe('extractAll on macOS (RB-097-02)', () => {
  const unix = { unix: true };

  it('keeps the executable bit and makes the framework links, after every file', async () => {
    const zip = makeZip([
      { name: 'WebKit.framework/Versions/Current', data: 'A', mode: 0o120755 },
      { name: 'WebKit.framework/WebKit', data: 'Versions/Current/WebKit', mode: 0o120755 },
      { name: 'WebKit.framework/Versions/A/WebKit', data: 'dylib', mode: 0o100644, deflate: true },
      { name: 'WebKit.framework/Versions/A/XPCServices/x.xpc', data: '../../../../x.xpc', mode: 0o120755 },
      { name: 'x.xpc/', mode: 0o40755 },
      { name: 'pw_run.sh', data: '#!/bin/sh', mode: 0o104755 },
    ]);
    expect(await extractAll(zip, dir, undefined, unix)).toBe(6);
    const script = await stat(join(dir, 'pw_run.sh'));
    expect(script.mode & 0o100).toBe(0o100);
    // Never setuid, whatever the archive says.
    expect(script.mode & 0o4000).toBe(0);
    expect((await stat(join(dir, 'WebKit.framework/Versions/A/WebKit'))).mode & 0o111).toBe(0);
    expect(await readlink(join(dir, 'WebKit.framework/Versions/Current'))).toBe('A');
    expect(await readFile(join(dir, 'WebKit.framework/WebKit'), 'utf8')).toBe('dylib');
    expect((await lstat(join(dir, 'WebKit.framework/Versions/A/XPCServices/x.xpc'))).isSymbolicLink()).toBe(true);
  });

  it('refuses a link that leaves the folder, also through another link, and writes nothing', async () => {
    const escapes = [
      [{ name: 'l', data: '/etc/passwd', mode: 0o120777 }],
      [{ name: 'a/l', data: '../../outside', mode: 0o120777 }],
      [{ name: 'l', data: 'a\\..\\..\\x', mode: 0o120777 }],
      // s/q points at the folder itself, so s/q/.. is its parent.
      [
        { name: 's/q', data: '..', mode: 0o120777 },
        { name: 'p', data: 's/q/..', mode: 0o120777 },
      ],
      [{ name: 'loop', data: 'loop/x', mode: 0o120777 }],
    ];
    for (const entries of escapes) {
      await expect(extractAll(makeZip([{ name: 'first', data: 'x' }, ...entries]), dir, undefined, unix), entries[0]?.name).rejects.toBeInstanceOf(ZipError);
      expect(await readdir(dir)).toEqual([]);
    }
  });

  it('refuses an entry under a link: nothing is written through one', async () => {
    const zip = makeZip([
      { name: 'l', data: 'd', mode: 0o120777 },
      { name: 'd/', mode: 0o40755 },
      { name: 'l/file', data: 'x', mode: 0o100644 },
    ]);
    await expect(extractAll(zip, dir, undefined, unix)).rejects.toThrow('under the link');
    expect(await readdir(dir)).toEqual([]);
  });

  it('writes a link as a plain file where links are not made (Windows)', async () => {
    const zip = makeZip([{ name: 'l', data: 'target', mode: 0o120777 }]);
    await extractAll(zip, dir);
    expect((await lstat(join(dir, 'l'))).isSymbolicLink()).toBe(false);
    expect(await readFile(join(dir, 'l'), 'utf8')).toBe('target');
  });
});

describe('safeName', () => {
  it('keeps relative paths with slashes', () => {
    expect(safeName('a\\b\\c.dll')).toBe('a/b/c.dll');
    expect(safeName('dir/')).toBe('dir/');
  });
});
