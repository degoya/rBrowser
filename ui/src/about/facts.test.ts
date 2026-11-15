import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import type { AboutInfo } from '../ipc/protocol';
import { shellMessages } from '../i18n';
import { formatMessage } from '../i18n/format';
import {
  APP_NAME,
  AUTHOR,
  engineRows,
  engineText,
  isWebLink,
  LICENSE,
  LINKS,
  REPOSITORY,
  versionInfo,
  type Translate,
} from './facts';

// The repository root: the tests run in ui/ (`npm test`).
const ROOT = join(process.cwd(), '..');
const cargoToml = readFileSync(join(ROOT, 'Cargo.toml'), 'utf-8');

function lookup(key: string): string {
  let node: unknown = shellMessages();
  for (const part of key.split('.')) {
    node = (node as Record<string, unknown> | undefined)?.[part];
  }
  return typeof node === 'string' ? node : key;
}

const t: Translate = (key, args = {}) => formatMessage(lookup(key), args);

const INFO: AboutInfo = {
  version: '0.1.0',
  commit: 'f98c7fa1',
  cef: '156.0.26+g8ea0c73',
  chromium: '156.0.8078.88',
  built: '2026-11-10T12:00:00Z',
  platform: 'windows x86_64',
  apis: { plugin: 'rbrowser:plugin@0.1.0', configSchema: '1' },
  dirs: { logs: 'C:/Users/someone/AppData/Local/rBrowser/logs' },
};

describe('the version info', () => {
  it('is a line for the build, then one per engine and API', () => {
    expect(versionInfo(INFO, t)).toBe(
      [
        'rBrowser 0.1.0 (commit f98c7fa1, built 2026-11-10T12:00:00Z, windows x86_64)',
        'CEF: 156.0.26+g8ea0c73',
        'Chromium: 156.0.8078.88',
        'Firefox: not found',
        'WebKit (Playwright): not installed',
        'Node: not detected',
        'Plugin API: rbrowser:plugin@0.1.0',
        'Config schema: 1',
      ].join('\n'),
    );
  });

  it('says unknown for what the build does not know and names an engine that is there', () => {
    const text = versionInfo({ ...INFO, commit: 'unknown', built: undefined, firefox: '146.0' }, t);
    expect(text.split('\n')[0]).toBe('rBrowser 0.1.0 (commit unknown, built unknown, windows x86_64)');
    expect(text).toContain('Firefox: 146.0');
  });

  it('leaves the folders out: they name the user account', () => {
    expect(versionInfo(INFO, t)).not.toContain('someone');
  });
});

describe('the engines', () => {
  it('lists CEF, Chromium, Firefox, WebKit and Node in that order', () => {
    expect(engineRows(INFO).map((row) => row.id)).toEqual(['cef', 'chromium', 'firefox', 'webkit', 'node']);
  });

  it('shows an absent engine by its absence', () => {
    const firefox = engineRows(INFO).find((row) => row.id === 'firefox');
    expect(firefox && engineText(firefox, t)).toBe('not found');
  });
});

describe('the facts kept beside Cargo.toml', () => {
  it('name the author, the licence and the repository of the workspace', () => {
    expect(cargoToml).toContain(`authors = ["${AUTHOR}"]`);
    expect(cargoToml).toContain(`license = "${LICENSE}"`);
    expect(cargoToml).toContain(`repository = "${REPOSITORY}"`);
    expect(APP_NAME).toBe('rBrowser');
  });

  it('offer only web addresses, none published before the first public export', () => {
    expect(LINKS.map((link) => link.kind)).toEqual(['source', 'handbook', 'changelog']);
    for (const link of LINKS) {
      expect(isWebLink(link.url)).toBe(true);
      expect(link.published).toBe(false);
    }
    expect(isWebLink('javascript:alert(1)')).toBe(false);
    expect(isWebLink('not a url')).toBe(false);
  });
});
