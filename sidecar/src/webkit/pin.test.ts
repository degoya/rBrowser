import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { ErrorCode } from '../rpc.ts';
import { PIN_MARKER, WEBKIT_PIN, archiveUrls, executableName } from './pin.ts';
import { installedExecutable } from './real.ts';

const require = createRequire(import.meta.url);
const packageDir = dirname(require.resolve('playwright-core/package.json'));

describe('WEBKIT_PIN', () => {
  it('belongs to the installed playwright-core and its WebKit revision', () => {
    const pkg = JSON.parse(readFileSync(join(packageDir, 'package.json'), 'utf8')) as { version: string };
    const browsers = JSON.parse(readFileSync(join(packageDir, 'browsers.json'), 'utf8')) as {
      browsers: Array<{ name: string; revision: string }>;
    };
    expect(pkg.version).toBe(WEBKIT_PIN.playwright);
    expect(browsers.browsers.find((b) => b.name === 'webkit')?.revision).toBe(WEBKIT_PIN.revision);
  });

  it('is a SHA-256 and HTTPS mirrors only', () => {
    expect(WEBKIT_PIN.sha256).toMatch(/^[0-9a-f]{64}$/);
    expect(archiveUrls().every((url) => url.startsWith('https://') && url.endsWith('/builds/webkit/2388/webkit-win64.zip'))).toBe(
      true,
    );
  });
});

describe('installedExecutable', () => {
  const entries = [{ name: 'webkit', revision: '2388', version: '27.2' }];

  const marker = join('/pw', 'webkit-2388', PIN_MARKER);
  const reading = (files: Record<string, string>) => (path: string) => files[path] ?? null;

  it('names Playwright.exe of a build rBrowser installed from the pinned archive', () => {
    expect(installedExecutable('/pw', entries, 'win32', reading({ [marker]: WEBKIT_PIN.sha256 }))).toBe(
      join('/pw', 'webkit-2388', executableName('win32')),
    );
  });

  it('says WebKit is not installed without the marker or a browsers folder', () => {
    expect(() => installedExecutable('/pw', entries, 'win32', () => null)).toThrow(
      expect.objectContaining({ code: ErrorCode.WebkitNotInstalled }),
    );
    expect(() => installedExecutable(undefined, entries, 'win32', () => WEBKIT_PIN.sha256)).toThrow(
      expect.objectContaining({ code: ErrorCode.WebkitNotInstalled }),
    );
  });

  it('trusts neither Playwright\'s own marker nor one naming another archive', () => {
    const playwrights = join('/pw', 'webkit-2388', 'INSTALLATION_COMPLETE');
    for (const files of [{ [playwrights]: '' }, { [marker]: '0'.repeat(64) }]) {
      expect(() => installedExecutable('/pw', entries, 'win32', reading(files))).toThrow(
        expect.objectContaining({ code: ErrorCode.WebkitNotInstalled }),
      );
    }
  });
});
