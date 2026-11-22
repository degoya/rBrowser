import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { ErrorCode } from '../rpc.ts';
import { parseBrowsersJson } from '../modules/playwright.ts';
import { MAC_PINS, PIN_MARKER, WEBKIT_PIN, archiveUrls, executableName, hostPlatform, pinFor } from './pin.ts';
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

describe('MAC_PINS (RB-097-02)', () => {
  const browsersJson = readFileSync(join(packageDir, 'browsers.json'), 'utf8');
  const bundle = readFileSync(join(packageDir, 'lib', 'coreBundle.js'), 'utf8');

  it('pins every macOS build the installed playwright-core downloads, at its revision for that host', () => {
    for (const [host, pin] of Object.entries(MAC_PINS)) {
      const webkit = parseBrowsersJson(browsersJson, host).find((b) => b.name === 'webkit');
      expect(pin.playwright, host).toBe(WEBKIT_PIN.playwright);
      expect(pin.revision, host).toBe(webkit?.revision);
      expect(bundle, host).toContain(`"${host}": "builds/webkit/%s/${pin.archive}"`);
      expect(pin.sha256, host).toMatch(/^[0-9a-f]{64}$/);
      expect(pin.bytes, host).toBeGreaterThan(0);
    }
    expect(MAC_PINS.mac14.revision).toBe('2251');
  });

  it('downloads each from the HTTPS mirrors under its own revision and name', () => {
    expect(archiveUrls(MAC_PINS['mac15-arm64']).every((url) => url.startsWith('https://') && url.endsWith('/builds/webkit/2388/webkit-mac-15-arm64.zip'))).toBe(true);
    expect(archiveUrls(MAC_PINS.mac14)[0]).toMatch(/\/builds\/webkit\/2251\/webkit-mac-14\.zip$/);
  });
});

describe('hostPlatform', () => {
  const intel = ['Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz'];
  const apple = ['Apple M2 Pro'];

  it('names Windows x64 win64 and nothing else on Windows', () => {
    expect(hostPlatform('win32', 'x64', '10.0.26100', intel)).toBe('win64');
    expect(hostPlatform('win32', 'arm64', '10.0.26100', intel)).toBeNull();
  });

  it('names macOS by its Darwin version as Playwright does, arm64 on Apple silicon', () => {
    expect(hostPlatform('darwin', 'arm64', '23.6.0', apple)).toBe('mac14-arm64');
    expect(hostPlatform('darwin', 'x64', '24.1.0', intel)).toBe('mac15');
    expect(hostPlatform('darwin', 'arm64', '25.0.0', apple)).toBe('mac26-arm64');
    // Newer than the last build: Playwright takes the last one.
    expect(hostPlatform('darwin', 'x64', '27.0.0', intel)).toBe('mac26');
  });

  it('has no build for macOS 13 and older, Linux or an unreadable version', () => {
    expect(hostPlatform('darwin', 'arm64', '22.6.0', apple)).toBeNull();
    expect(hostPlatform('darwin', 'arm64', 'unknown', apple)).toBeNull();
    expect(hostPlatform('linux', 'x64', '6.8.0', intel)).toBeNull();
  });
});

describe('pinFor', () => {
  it('is the Windows pin on win64, the macOS pin of the host, else none', () => {
    expect(pinFor('win64')).toBe(WEBKIT_PIN);
    expect(pinFor('mac15-arm64')).toBe(MAC_PINS['mac15-arm64']);
    expect(pinFor(null)).toBeNull();
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

  it('names pw_run.sh of a macOS build installed from its pin (RB-097-02)', () => {
    const mac = [{ name: 'webkit', revision: '2251', version: '27.2' }];
    const pin = MAC_PINS.mac14;
    const files = { [join('/pw', 'webkit-2251', PIN_MARKER)]: pin.sha256 };
    expect(installedExecutable('/pw', mac, 'darwin', reading(files), pin)).toBe(join('/pw', 'webkit-2251', 'pw_run.sh'));
    // The Windows archive's marker is no macOS build.
    const windows = { [join('/pw', 'webkit-2251', PIN_MARKER)]: WEBKIT_PIN.sha256 };
    expect(() => installedExecutable('/pw', mac, 'darwin', reading(windows), pin)).toThrow(
      expect.objectContaining({ code: ErrorCode.WebkitNotInstalled }),
    );
    expect(() => installedExecutable('/pw', mac, 'darwin', reading(files), null)).toThrow(
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
