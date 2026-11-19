// Settings > Plugins > Catalog, the bundled plugins (RB-091-04, as rDownloader's catalogue): every
// first-party plugin the package carries, chosen or not, with the update a repository offers
// under the bundle's key (`plugins/bundle_offer.rs`, `bundled_rows`). A plugin not chosen installs
// with one click (`installChosen`: from the repository when it offers a newer version than the
// bundle's, from the bundle otherwise); a chosen one can be taken out (`removeChosen`) and does
// not come back by itself; an update opens the review of the repository's package. Read
// defensively; every text is shown as text.

export interface BundledUpdate {
  repository: string;
  version: string;
}

export interface BundledPlugin {
  id: string;
  name: string;
  description: string;
  version: string;
  kinds: string[];
  category: string;
  sizeBytes: number;
  recommended: boolean;
  grants: string[];
  installed: boolean;
  update: BundledUpdate | null;
}

function record(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
}

function text(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function texts(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];
}

/** The core's `bundled` rows; one without an id is dropped. */
export function parseBundled(value: unknown): BundledPlugin[] {
  if (!Array.isArray(value)) return [];
  const plugins: BundledPlugin[] = [];
  for (const item of value) {
    const row = record(item);
    const id = text(row.id);
    if (id === '') continue;
    const update = record(row.update);
    plugins.push({
      id,
      name: text(row.name),
      description: text(row.description),
      version: text(row.version),
      kinds: texts(row.kinds),
      category: text(row.category),
      sizeBytes: typeof row.sizeBytes === 'number' ? row.sizeBytes : 0,
      recommended: row.recommended === true,
      grants: texts(row.grants),
      installed: row.installed === true,
      update: typeof update.repository === 'string' && typeof update.version === 'string' ? { repository: update.repository, version: update.version } : null,
    });
  }
  return plugins;
}

/** The repository id a bundled row stands for when no update names one. */
export const BUNDLE_ORIGIN = 'bundle';

/** The first failure's code of an `installChosen` or `removeChosen` answer; `null` when every
 * plugin went through. */
export function chosenFailure(message: unknown): string | null {
  const results = record(record(message).value).results;
  if (!Array.isArray(results)) return null;
  for (const item of results) {
    const result = record(item);
    if (result.ok !== true) return text(result.error) || 'plugins.bundle.error.unknown';
  }
  return null;
}
