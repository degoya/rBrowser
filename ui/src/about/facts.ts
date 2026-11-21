// What the About page shows beside the core's facts (RB-010-18): the name, the author, the
// licences and the addresses, each held to `Cargo.toml` by `facts.test.ts`, and the version
// block "Copy version info" puts on the clipboard (format after rCommander's RC-010-19).

import type { AboutInfo } from '../ipc/protocol';

export const APP_NAME = 'rBrowser';
/** `[workspace.package] authors`. */
export const AUTHOR = 'Alexander Herling / DEGOYA.digital';
/** `[workspace.package] license`. */
export const LICENSE = 'GPL-3.0-or-later';
/** The plugin SDK templates, the WIT contract and the SDK CI workflow. */
export const SDK_LICENSE = 'MIT';
/** `[workspace.package] repository`: the public mirror on GitHub (RB-090-12). */
export const REPOSITORY = 'https://github.com/degoya/rBrowser';

export type LinkKind = 'source' | 'handbook' | 'changelog';

export interface AboutLink {
  kind: LinkKind;
  url: string;
  /** False until the first public export (RB-090-12): the page marks it "not yet published". */
  published: boolean;
}

export const LINKS: AboutLink[] = [
  { kind: 'source', url: REPOSITORY, published: false },
  { kind: 'handbook', url: `${REPOSITORY}/wiki`, published: false },
  { kind: 'changelog', url: `${REPOSITORY}/blob/main/CHANGELOG.md`, published: false },
];

/** Only these open in a view or go to the clipboard. */
export function isWebLink(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.protocol === 'https:' || parsed.protocol === 'http:';
  } catch {
    return false;
  }
}

export type Translate = (key: string, args?: Record<string, unknown>) => string;

export type EngineId = 'cef' | 'chromium' | 'firefox' | 'webkit' | 'safari' | 'node';

export interface EngineRow {
  id: EngineId;
  name: string;
  /** The version, or `null` when absent. */
  version: string | null;
  /** The i18n key of what an absent engine reads as. */
  absentKey: string;
}

/** The engine rows in the page's order; Safari only where the core names it (macOS). */
export function engineRows(info: AboutInfo): EngineRow[] {
  const safari: EngineRow[] =
    info.safari === undefined
      ? []
      : [{ id: 'safari', name: 'Safari', version: info.safari, absentKey: 'settings.about.engine.not_found' }];
  return [
    { id: 'cef', name: 'CEF', version: info.cef, absentKey: 'settings.about.engine.unknown' },
    { id: 'chromium', name: 'Chromium', version: info.chromium, absentKey: 'settings.about.engine.unknown' },
    { id: 'firefox', name: 'Firefox', version: info.firefox ?? null, absentKey: 'settings.about.engine.not_found' },
    {
      id: 'webkit',
      name: 'WebKit (Playwright)',
      version: info.webkit ?? null,
      absentKey: 'settings.about.engine.not_installed',
    },
    ...safari,
    { id: 'node', name: 'Node', version: info.node ?? null, absentKey: 'settings.about.engine.not_detected' },
  ];
}

/** What a fact the build does not know reads as: the core's `unknown`, or nothing. */
function orUnknown(value: string | undefined, t: Translate): string {
  return value === undefined || value === '' || value === 'unknown' ? t('settings.about.unknown') : value;
}

/** The engine's version, or what its absence reads as. */
export function engineText(row: EngineRow, t: Translate): string {
  return row.version === null || row.version === '' || row.version === 'unknown' ? t(row.absentKey) : row.version;
}

/** The plugin contract and the config schema, in the page's order. */
export function apiRows(info: AboutInfo): { key: string; value: string }[] {
  const apis = info.apis ?? {};
  return ['plugin', 'mcp', 'configSchema']
    .filter((name) => apis[name] !== undefined)
    .map((name) => ({ key: `settings.about.api.${name}`, value: apis[name] ?? '' }));
}

/**
 * The block for a bug report: `{name} {version} (commit {commit}, built {built}, {platform})`,
 * then a line per engine and per API. The folders stay out: they name the user's account.
 */
export function versionInfo(info: AboutInfo, t: Translate): string {
  const head = t('settings.about.version_line', {
    name: APP_NAME,
    version: info.version,
    commit: orUnknown(info.commit, t),
    built: orUnknown(info.built, t),
    platform: orUnknown(info.platform, t),
  });
  const engines = engineRows(info).map((row) =>
    t('settings.about.copy_line', { label: row.name, value: engineText(row, t) }),
  );
  const apis = apiRows(info).map((row) => t('settings.about.copy_line', { label: t(row.key), value: row.value }));
  return [head, ...engines, ...apis].join('\n');
}
