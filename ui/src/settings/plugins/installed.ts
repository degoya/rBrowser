// The installed plugins as the core reports them (`pluginPanel { op: 'installed' }`, written by
// `rbrowser::plugins::list`): read at the boundary, so a row the window does not understand is
// dropped instead of rendered. A verified row carries the plugin's id and its manifest's texts;
// a refused package has no id - it is named by its file, and what its manifest claims
// (`declared`) is unverified text. Every text is shown as text only.

import { isObject } from '../../../../plugins/web-common/src/is-object';

export const SOURCES = ['bundled', 'user', 'project'] as const;
export type PluginSourceId = (typeof SOURCES)[number];

export const SIGNATURES = ['signed', 'unsigned', 'invalid', 'untrusted', 'unknown'] as const;
export type SignatureState = (typeof SIGNATURES)[number];

export const STATES = ['active', 'disabled', 'suspended', 'blocked', 'error'] as const;
export type PluginState = (typeof STATES)[number];

/** What a refused package says of itself: not verified. */
export interface DeclaredPlugin {
  name: string;
  version: string;
  /** The name of the installed plugin whose id the package claims. */
  claimsIdOf?: string;
}

export interface InstalledPlugin {
  /** The id of a verified plugin; empty for a refused package. */
  id: string;
  /** The package's file name, for a refused package. */
  file: string;
  /** Whether the package verified: only then is `id` an id a setting may name. */
  verified: boolean;
  /** The manifest's name of a verified plugin; the file name of a refused package. */
  name: string;
  version: string;
  description: string;
  declared?: DeclaredPlugin;
  source: PluginSourceId;
  signature: { state: SignatureState; keyId?: string };
  state: PluginState;
  /** Why it did not load or does not run: the stable code and the text. */
  error?: { code: string; message: string };
  /** How many commands its manifest declares (`plugin.<id>.*`). */
  commands: number;
}

type Folders = Partial<Record<PluginSourceId, string>>;

export interface InstalledList {
  /** The plugins are still loading: the list follows. */
  loading: boolean;
  plugins: InstalledPlugin[];
  /** Per source the folder the start read. */
  folders: Folders;
  /** Per source the folder the start looked for and did not find. */
  missing: Folders;
}

function oneOf<T extends string>(value: unknown, allowed: readonly T[]): T | null {
  return typeof value === 'string' && (allowed as readonly string[]).includes(value) ? (value as T) : null;
}

function text(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function parsePlugin(value: unknown): InstalledPlugin | null {
  if (!isObject(value)) {
    return null;
  }
  const source = oneOf(value.source, SOURCES);
  const state = oneOf(value.state, STATES);
  const verified = value.verified === true;
  const id = verified ? text(value.id) : '';
  const file = text(value.file);
  // A verified row needs its id, a refused one its file; anything else names nothing.
  if (source === null || state === null || (verified ? id === '' : file === '')) {
    return null;
  }
  const signature = isObject(value.signature) ? value.signature : {};
  const plugin: InstalledPlugin = {
    id,
    file,
    verified,
    name: verified ? text(value.name) || id : file,
    version: verified ? text(value.version) : '',
    description: verified ? text(value.description) : '',
    source,
    signature: { state: oneOf(signature.state, SIGNATURES) ?? 'unknown' },
    state,
    commands: verified && typeof value.commands === 'number' && value.commands > 0 ? Math.floor(value.commands) : 0,
  };
  if (verified && typeof signature.keyId === 'string' && signature.keyId !== '') {
    plugin.signature.keyId = signature.keyId;
  }
  if (!verified && isObject(value.declared)) {
    plugin.declared = { name: text(value.declared.name), version: text(value.declared.version) };
    if (typeof value.declared.claimsIdOf === 'string' && value.declared.claimsIdOf !== '') {
      plugin.declared.claimsIdOf = value.declared.claimsIdOf;
    }
  }
  if (isObject(value.error) && typeof value.error.code === 'string') {
    plugin.error = { code: value.error.code, message: text(value.error.message) || value.error.code };
  }
  return plugin;
}

function parseFolders(value: unknown): Folders {
  const folders: Folders = {};
  if (isObject(value)) {
    for (const source of SOURCES) {
      const folder = value[source];
      if (typeof folder === 'string' && folder !== '') {
        folders[source] = folder;
      }
    }
  }
  return folders;
}

/** The list in `message`; anything malformed reads as no plugins. */
export function parseInstalled(message: unknown): InstalledList {
  if (!isObject(message)) {
    return { loading: false, plugins: [], folders: {}, missing: {} };
  }
  const rows: unknown[] = Array.isArray(message.plugins) ? message.plugins : [];
  return {
    loading: message.loading === true,
    plugins: rows.flatMap((row) => {
      const plugin = parsePlugin(row);
      return plugin === null ? [] : [plugin];
    }),
    folders: parseFolders(message.folders),
    missing: parseFolders(message.missing),
  };
}

/** What names a row on the page: the id of a verified plugin, the file of a refused package. */
export function rowName(plugin: InstalledPlugin): string {
  return plugin.verified ? plugin.id : plugin.file;
}

/** Whether `plugins.disabled` may name the row: a verified plugin that loads or is switched
 * off. A refused or waiting package has no switch - what it claims is no id. */
export function switchable(plugin: InstalledPlugin): boolean {
  return plugin.verified && (plugin.state === 'active' || plugin.state === 'disabled' || plugin.state === 'suspended');
}

/** The filter of the shortcut editor that shows the commands of plugin `id`. */
export function commandQuery(id: string): string {
  return `plugin.${id}.`;
}
