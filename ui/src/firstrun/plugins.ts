// The plugin step's logic (RB-091-05, after rDownloader's "Your services"): what RB-091-04's
// `available` answers - the installable first-party plugins with name, description, category,
// size, recommended, origin and grants - read defensively, grouped by category, filtered by
// category and search, the recommended and the installed ones ticked; what the choice installs
// and removes, its size and grants, shown before the one confirmation; the progress and outcome.
// The wizard only marks what gets loaded: at a fresh install the core keeps the choice pending
// until the step answers (or `applyRecommended` when it is skipped).

export type PluginCategory = 'checks' | 'devtools' | 'overlays' | 'capture' | 'tools';
export type CategoryFilter = PluginCategory | 'all';

/** The categories in the order the step shows them. */
export const CATEGORIES: readonly PluginCategory[] = ['checks', 'devtools', 'overlays', 'capture', 'tools'];

export interface AvailablePlugin {
  id: string;
  name: string;
  description: string;
  category: PluginCategory;
  sizeBytes: number;
  recommended: boolean;
  /** Where the install takes it from: the repository, or the bundle in the package. */
  origin: 'bundle' | 'repository';
  /** Capability ids (`views`, `cdp`, ...), named by `plugins.capability.<id>`. */
  grants: string[];
  installed: boolean;
  version: string;
}

export interface Available {
  /** No network: everything installs from the bundle in the package. */
  offline: boolean;
  /** A fresh install's choice not made yet: no optional plugin loads until the step answers
   * (`installChosen` or `removeChosen`) or is skipped (`applyRecommended`); every row says
   * not installed then. */
  choicePending: boolean;
  plugins: AvailablePlugin[];
}

export interface InstallProgress {
  id: string;
  done: number;
  total: number;
  state: 'installing' | 'installed' | 'failed';
  error?: string;
}

export interface InstallResult {
  id: string;
  ok: boolean;
  error?: string;
  origin: 'bundle' | 'repository';
}

/** What the plugin step did: the install's outcome (and failed removals), the removed ids. */
export type PluginsDone = InstallOutcome & { removed: string[] };

export interface InstallOutcome {
  results: InstallResult[];
  /** The plugins load at the next start. */
  restart: boolean;
}

type Json = Record<string, unknown>;

function isObject(value: unknown): value is Json {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

const isString = (value: unknown): value is string => typeof value === 'string';

function parsePlugin(value: unknown): AvailablePlugin | null {
  if (!isObject(value)) return null;
  const { id, name, description, category, sizeBytes, recommended, origin, grants, installed, version } = value;
  if (
    !isString(id) ||
    !isString(name) ||
    !isString(description) ||
    !CATEGORIES.includes(category as PluginCategory) ||
    typeof sizeBytes !== 'number' ||
    (origin !== 'bundle' && origin !== 'repository') ||
    !Array.isArray(grants) ||
    !grants.every(isString)
  ) {
    return null;
  }
  return {
    id,
    name,
    description,
    category: category as PluginCategory,
    sizeBytes,
    recommended: recommended === true,
    origin,
    grants,
    installed: installed === true,
    version: isString(version) ? version : '',
  };
}

/** The `available` answer's value; `null` when it is not one. A malformed entry is left out. */
export function parseAvailable(value: unknown): Available | null {
  if (!isObject(value) || !Array.isArray(value.plugins)) return null;
  return {
    offline: value.offline === true,
    choicePending: value.choicePending === true,
    plugins: value.plugins.map(parsePlugin).filter((plugin): plugin is AvailablePlugin => plugin !== null),
  };
}

/** What is ticked at first: with the first choice pending the recommended plugins, else what is
 * installed. The choice is the set wanted after the step; anything can be unticked, an installed
 * plugin included. */
export function initialChoice(available: Available): string[] {
  return available.plugins
    .filter((plugin) => (available.choicePending ? plugin.recommended : plugin.installed))
    .map((plugin) => plugin.id);
}

/** What the confirmation does: install the ticked plugins not installed, remove the installed
 * ones not ticked (rDownloader RD-180-14). */
export function changesOf(
  plugins: readonly AvailablePlugin[],
  chosen: readonly string[],
): { install: string[]; remove: string[] } {
  return {
    install: plugins.filter((plugin) => !plugin.installed && chosen.includes(plugin.id)).map((plugin) => plugin.id),
    remove: plugins.filter((plugin) => plugin.installed && !chosen.includes(plugin.id)).map((plugin) => plugin.id),
  };
}

/** The plugins per category, in [`CATEGORIES`] order; empty groups are left out. */
export function groupByCategory(plugins: readonly AvailablePlugin[]): [PluginCategory, AvailablePlugin[]][] {
  return CATEGORIES.map((category): [PluginCategory, AvailablePlugin[]] => [
    category,
    plugins.filter((plugin) => plugin.category === category),
  ]).filter(([, group]) => group.length > 0);
}

/** The plugins of `category` whose name or description holds `query` (any case). */
export function visiblePlugins(
  plugins: readonly AvailablePlugin[],
  category: CategoryFilter,
  query: string,
): AvailablePlugin[] {
  const needle = query.trim().toLowerCase();
  return plugins.filter(
    (plugin) =>
      (category === 'all' || plugin.category === category) &&
      (needle === '' || `${plugin.name} ${plugin.description}`.toLowerCase().includes(needle)),
  );
}

/** `chosen` with every visible plugin. */
export function withAll(chosen: readonly string[], visible: readonly AvailablePlugin[]): string[] {
  const added = visible.filter((plugin) => !chosen.includes(plugin.id)).map((plugin) => plugin.id);
  return [...chosen, ...added];
}

/** `chosen` without the visible plugins. */
export function withNone(chosen: readonly string[], visible: readonly AvailablePlugin[]): string[] {
  const hidden = new Set(visible.map((plugin) => plugin.id));
  return chosen.filter((id) => !hidden.has(id));
}

/** The download size of what the choice installs, in bytes. */
export function totalBytes(plugins: readonly AvailablePlugin[], chosen: readonly string[]): number {
  return plugins
    .filter((plugin) => !plugin.installed && chosen.includes(plugin.id))
    .reduce((sum, plugin) => sum + plugin.sizeBytes, 0);
}

/** Every grant of what the choice installs, once each, sorted. */
export function grantsOf(plugins: readonly AvailablePlugin[], chosen: readonly string[]): string[] {
  const grants = new Set(
    plugins.filter((plugin) => !plugin.installed && chosen.includes(plugin.id)).flatMap((plugin) => plugin.grants),
  );
  return [...grants].sort();
}

/** Megabytes with one decimal, for `firstrun.plugins.size`. */
export function megabytes(bytes: number): string {
  return (bytes / 1_000_000).toFixed(1);
}

/** The install's progress from a `pluginPanel` event's message; `null` for any other. */
export function parseProgress(message: unknown): InstallProgress | null {
  if (!isObject(message) || !isObject(message.pluginInstallProgress)) return null;
  const { id, done, total, state, error } = message.pluginInstallProgress;
  if (!isString(id) || typeof done !== 'number' || typeof total !== 'number') return null;
  if (state !== 'installing' && state !== 'installed' && state !== 'failed') return null;
  return { id, done, total, state, ...(isString(error) ? { error } : {}) };
}

/** The `installChosen` answer's value; `null` when it is not one. */
export function parseOutcome(value: unknown): InstallOutcome | null {
  if (!isObject(value) || !Array.isArray(value.results)) return null;
  const results = value.results.flatMap((entry): InstallResult[] => {
    if (!isObject(entry) || !isString(entry.id) || typeof entry.ok !== 'boolean') return [];
    const origin = entry.origin === 'repository' ? 'repository' : 'bundle';
    return [{ id: entry.id, ok: entry.ok, ...(isString(entry.error) ? { error: entry.error } : {}), origin }];
  });
  return { results, restart: value.restart === true };
}
