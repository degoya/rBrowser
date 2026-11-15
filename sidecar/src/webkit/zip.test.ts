import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
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

describe('safeName', () => {
  it('keeps relative paths with slashes', () => {
    expect(safeName('a\\b\\c.dll')).toBe('a/b/c.dll');
    expect(safeName('dir/')).toBe('dir/');
  });
});
