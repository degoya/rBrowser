import { describe, expect, it } from 'vitest';

import { grantForms } from './grants.ts';
import { nodeArgs } from './spawn.ts';

describe('grantForms', () => {
  it('grants a WSL share on Windows also in the \\\\?\\UNC\\ form Node checks a removal against', () => {
    expect(grantForms('\\\\wsl.localhost\\debian\\home\\dev\\site', 'win32')).toEqual([
      '\\\\wsl.localhost\\debian\\home\\dev\\site',
      '\\\\?\\UNC\\wsl.localhost\\debian\\home\\dev\\site',
    ]);
  });

  it('grants a drive path on Windows also in the \\\\?\\ form', () => {
    expect(grantForms('C:\\Tools\\rbrowser\\sidecar', 'win32')).toEqual([
      'C:\\Tools\\rbrowser\\sidecar',
      '\\\\?\\C:\\Tools\\rbrowser\\sidecar',
    ]);
  });

  it('turns mixed separators into backslashes on Windows', () => {
    expect(grantForms('\\\\wsl.localhost\\debian\\site\\.rbrowser/out/flows\\20261117-1', 'win32')).toEqual([
      '\\\\wsl.localhost\\debian\\site\\.rbrowser\\out\\flows\\20261117-1',
      '\\\\?\\UNC\\wsl.localhost\\debian\\site\\.rbrowser\\out\\flows\\20261117-1',
    ]);
  });

  it('grants a path given in the long form also in the plain form', () => {
    expect(grantForms('\\\\?\\UNC\\wsl.localhost\\debian\\site', 'win32')).toEqual([
      '\\\\wsl.localhost\\debian\\site',
      '\\\\?\\UNC\\wsl.localhost\\debian\\site',
    ]);
    expect(grantForms('\\\\?\\C:\\site', 'win32')).toEqual(['C:\\site', '\\\\?\\C:\\site']);
  });

  it('leaves a path on Linux and macOS as it is', () => {
    expect(grantForms('/site/.rbrowser/out', 'linux')).toEqual(['/site/.rbrowser/out']);
    expect(grantForms('/site', 'darwin')).toEqual(['/site']);
  });
});

describe('nodeArgs on Windows', () => {
  const deps = { sidecarDir: 'C:\\Tools\\rbrowser\\sidecar', workerScript: 'C:\\w.js', netFlag: false };

  it("grants every folder in both forms, for writing only the run's local temp folder", () => {
    const tmp = 'C:\\Users\\me\\AppData\\Local\\rBrowser\\flows-tmp\\r';
    const args = nodeArgs(deps, '\\\\wsl.localhost\\debian\\site', tmp, 'win32');
    const reads = args.filter((a) => a.startsWith('--allow-fs-read=')).map((a) => a.slice('--allow-fs-read='.length));
    const writes = args.filter((a) => a.startsWith('--allow-fs-write=')).map((a) => a.slice('--allow-fs-write='.length));
    expect(reads).toEqual([
      'C:\\Tools\\rbrowser\\sidecar',
      '\\\\?\\C:\\Tools\\rbrowser\\sidecar',
      '\\\\wsl.localhost\\debian\\site',
      '\\\\?\\UNC\\wsl.localhost\\debian\\site',
      tmp,
      `\\\\?\\${tmp}`,
    ]);
    expect(writes).toEqual([tmp, `\\\\?\\${tmp}`]);
    expect(writes.some((w) => w.includes('wsl.localhost'))).toBe(false);
  });
});

// RB-097-03: macOS's own folders are symbolic links (`/tmp`, `/var` into `/private`), and Node's
// permission model compares a path as given, so a realpath'd access would miss a plain grant.
describe('grants on macOS', () => {
  const real = (path: string) => path.replace(/^\/(tmp|var)(?=\/|$)/, '/private/$1');

  it('grants a folder also under its real path when that differs', () => {
    expect(grantForms('/var/folders/x/T/rbrowser-flows-tmp/r', 'darwin', real)).toEqual([
      '/var/folders/x/T/rbrowser-flows-tmp/r',
      '/private/var/folders/x/T/rbrowser-flows-tmp/r',
    ]);
    expect(grantForms('/Users/me/site', 'darwin', real)).toEqual(['/Users/me/site']);
  });

  it('resolves only on macOS', () => {
    expect(grantForms('/tmp/site', 'linux', real)).toEqual(['/tmp/site']);
    expect(grantForms('C:\\site', 'win32', real)).toEqual(['C:\\site', '\\\\?\\C:\\site']);
  });

  it('keeps the plain grant when the real path cannot be found', () => {
    const fails = () => {
      throw new Error('ENOENT');
    };
    expect(grantForms('/tmp/site', 'darwin', fails)).toEqual(['/tmp/site']);
  });

  it('grants the project, the sidecar and the temp folder in both forms, writing only the temp folder', () => {
    const deps = { sidecarDir: '/Applications/rBrowser.app/Contents/Resources/sidecar', workerScript: '/w.js', netFlag: true, realPath: real };
    const args = nodeArgs(deps, '/tmp/site', '/var/t/r', 'darwin');
    const reads = args.filter((a) => a.startsWith('--allow-fs-read=')).map((a) => a.slice('--allow-fs-read='.length));
    const writes = args.filter((a) => a.startsWith('--allow-fs-write=')).map((a) => a.slice('--allow-fs-write='.length));
    expect(reads).toEqual([deps.sidecarDir, '/tmp/site', '/private/tmp/site', '/var/t/r', '/private/var/t/r']);
    expect(writes).toEqual(['/var/t/r', '/private/var/t/r']);
  });
});
