import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { SEED_SETTINGS, STAGE_MARKER, installOf, prepareData, taskCount } from './app.ts';

function staged(): string {
  const dir = mkdtempSync(join(tmpdir(), 'rbrowser-e2e-'));
  writeFileSync(join(dir, 'rbrowser.exe'), '');
  writeFileSync(join(dir, 'rbrowser.com'), '');
  writeFileSync(join(dir, STAGE_MARKER), '');
  return dir;
}

describe('installOf', () => {
  it('names the executables and the portable folders of a staged app', () => {
    const install = installOf('/c/app', 'native');
    expect(install).toMatchObject({
      dir: '/c/app',
      exe: join('/c/app', 'rbrowser.exe'),
      com: join('/c/app', 'rbrowser.com'),
      data: join('/c/app', 'data'),
      instances: join('/c/app', 'data', 'local', 'mcp'),
    });
  });
});

describe('prepareData', () => {
  it('switches the portable mode on and starts from the seeded settings only', () => {
    const dir = staged();
    mkdirSync(join(dir, 'data', 'local'), { recursive: true });
    writeFileSync(join(dir, 'data', 'trust.json'), '{"left":"over"}');
    prepareData(installOf(dir, 'native'));
    expect(existsSync(join(dir, 'portable'))).toBe(true);
    expect(existsSync(join(dir, 'data', 'trust.json'))).toBe(false);
    const settings = JSON.parse(readFileSync(join(dir, 'data', 'settings.json'), 'utf8'));
    expect(settings).toEqual(SEED_SETTINGS);
  });

  it('turns on what the run drives and nothing that could reach out', () => {
    expect(SEED_SETTINGS).toMatchObject({
      version: 1,
      mcp: { enabled: true, allowEvaluate: true },
      cdp: { enabled: true },
      performance: { counters: true },
      updates: { enabled: false },
      ui: { firstRunWizard: false },
    });
  });

  it('refuses a folder the e2e script did not stage: it would wipe a person data', () => {
    const dir = mkdtempSync(join(tmpdir(), 'portable-'));
    writeFileSync(join(dir, 'rbrowser.exe'), '');
    expect(() => prepareData(installOf(dir, 'native'))).toThrow(/not staged by scripts\/e2e-windows.sh/);
  });
});

describe('taskCount', () => {
  it('counts the rows of tasklist /FO CSV /NH output', () => {
    const csv = '"rbrowser.exe","1204","Console","1","120,000 K"\r\n"rbrowser.exe","1300","Console","1","80,000 K"\r\n';
    expect(taskCount(csv)).toBe(2);
    expect(taskCount('INFO: No tasks are running which match the specified criteria.\r\n')).toBe(0);
  });
});
