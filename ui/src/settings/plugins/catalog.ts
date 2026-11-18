// Settings > Plugins > Catalog (RB-091-01): the signed plugin catalogue as the core reports it
// (`pluginPanel { op: 'catalog' }`), read defensively - a field of the wrong type is dropped,
// never trusted - and the rows the page lists. One row per plugin and repository: the same id
// offered by two repositories is two rows, each naming its origin, and an install names the
// repository, id and version of its row. An update is offered from the repository the plugin
// was installed from only. Every text is shown as text, never as HTML.

import type { PluginGrantDetails } from '../../ipc/protocol';
import { BUNDLE_ORIGIN, type BundledPlugin, parseBundled } from './bundledRows';

export const KINDS = ['feature-plugin', 'check-plugin', 'analyzer-plugin', 'export-plugin'] as const;
export type Kind = (typeof KINDS)[number];
export type KindFilter = Kind | 'all';
export const SHOW = ['all', 'installed', 'updates'] as const;
export type ShowFilter = (typeof SHOW)[number];

export const KIND_KEYS: Record<KindFilter, string> = {
  all: 'settings.plugins.catalog.kind.all',
  'feature-plugin': 'settings.plugins.catalog.kind.feature',
  'check-plugin': 'settings.plugins.catalog.kind.check',
  'analyzer-plugin': 'settings.plugins.catalog.kind.analyzer',
  'export-plugin': 'settings.plugins.catalog.kind.export',
};

export const SHOW_KEYS: Record<ShowFilter, string> = {
  all: 'settings.plugins.catalog.show.all',
  installed: 'settings.plugins.catalog.show.installed',
  updates: 'settings.plugins.catalog.show.updates',
};

export interface Publisher {
  keyId: string;
  fingerprint: string;
  author: string;
}

export interface CatalogPackage {
  id: string;
  name: string;
  version: string;
  kinds: string[];
  description: string;
  capabilities: string[];
  minAppVersion: string | null;
  size: number;
  publisher: Publisher;
  releaseNotes: string | null;
}

export type Compatibility =
  | { state: 'compatible' }
  | { state: 'needsNewerApp'; minAppVersion: string }
  | { state: 'apiVersion'; apiVersion: string };

export interface Offer {
  repository: string;
  entry: CatalogPackage;
  compatibility: Compatibility;
}

export interface CatalogRepositoryInfo {
  id: string;
  name: string;
  url: string;
  official: boolean;
  fingerprint: string | null;
  loaded: boolean;
  error: string | null;
}

export interface RefusedRepository {
  name: string;
  url: string;
  error: string;
}

export interface CatalogInstall {
  repository: string;
  version: string;
}

export interface InstalledVersion {
  version: string;
  source: string;
}

export interface Catalog {
  repositories: CatalogRepositoryInfo[];
  refused: RefusedRepository[];
  entries: { id: string; offers: Offer[] }[];
  updates: Offer[];
  /** What the catalogue installed, by plugin id. */
  installs: Record<string, CatalogInstall>;
  /** What the start loaded, by plugin id; `null` while it loads. */
  installed: Record<string, InstalledVersion> | null;
  /** The first-party plugins the package carries, chosen or not (RB-091-04). */
  bundled: BundledPlugin[];
}

type Json = unknown;

function record(value: Json): Record<string, Json> {
  return value !== null && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, Json>) : {};
}

function text(value: Json): string {
  return typeof value === 'string' ? value : '';
}

function textOrNull(value: Json): string | null {
  return typeof value === 'string' ? value : null;
}

function texts(value: Json): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];
}

function list(value: Json): Json[] {
  return Array.isArray(value) ? value : [];
}

function parsePackage(value: Json): CatalogPackage {
  const entry = record(value);
  const publisher = record(entry.publisher);
  return {
    id: text(entry.id),
    name: text(entry.name),
    version: text(entry.version),
    kinds: texts(entry.kinds),
    description: text(entry.description),
    capabilities: texts(entry.capabilities),
    minAppVersion: textOrNull(entry.min_app_version),
    size: typeof entry.size === 'number' ? entry.size : 0,
    publisher: { keyId: text(publisher.key_id), fingerprint: text(publisher.fingerprint), author: text(publisher.author) },
    releaseNotes: textOrNull(entry.release_notes),
  };
}

function parseCompatibility(value: Json): Compatibility {
  const compatibility = record(value);
  switch (compatibility.state) {
    case 'compatible':
      return { state: 'compatible' };
    case 'needsNewerApp':
      return { state: 'needsNewerApp', minAppVersion: text(compatibility.minAppVersion) };
    default:
      return { state: 'apiVersion', apiVersion: text(compatibility.apiVersion) };
  }
}

function parseOffer(value: Json): Offer {
  const offer = record(value);
  return { repository: text(offer.repository), entry: parsePackage(offer.entry), compatibility: parseCompatibility(offer.compatibility) };
}

/** The core's answer as a catalogue; anything malformed reads as empty. */
export function parseCatalog(message: Json): Catalog {
  const answer = record(message);
  const installs: Record<string, CatalogInstall> = {};
  for (const [id, value] of Object.entries(record(answer.installs))) {
    const install = record(value);
    installs[id] = { repository: text(install.repository), version: text(install.version) };
  }
  let installed: Record<string, InstalledVersion> | null = null;
  if (answer.installed !== null && answer.installed !== undefined) {
    installed = {};
    for (const [id, value] of Object.entries(record(answer.installed))) {
      const row = record(value);
      installed[id] = { version: text(row.version), source: text(row.source) };
    }
  }
  return {
    repositories: list(answer.repositories).map((value) => {
      const repository = record(value);
      return {
        id: text(repository.id),
        name: text(repository.name),
        url: text(repository.url),
        official: repository.official === true,
        fingerprint: textOrNull(repository.fingerprint),
        loaded: repository.loaded === true,
        error: textOrNull(repository.error),
      };
    }),
    refused: list(answer.refused).map((value) => {
      const refused = record(value);
      return { name: text(refused.name), url: text(refused.url), error: text(refused.error) };
    }),
    entries: list(answer.entries).map((value) => {
      const entry = record(value);
      return { id: text(entry.id), offers: list(entry.offers).map(parseOffer) };
    }),
    updates: list(answer.updates).map(parseOffer),
    installs,
    installed,
    bundled: parseBundled(answer.bundled),
  };
}

/** What a row offers to do: a bundled plugin is chosen with one click and taken out again
 * (`choose`, `unchoose`, RB-091-04). */
export type RowAction = 'install' | 'update' | 'remove' | 'choose' | 'unchoose' | 'none';

export interface CatalogRow {
  /** `<repository>/<id>`: unique per row. */
  key: string;
  offer: Offer;
  repositoryName: string;
  /** Another repository offers the same id: the row names its origin. */
  severalOrigins: boolean;
  installedVersion: string | null;
  /** The catalogue installed it from this row's repository. */
  fromHere: boolean;
  /** It ships with rBrowser (RB-091-04): the row is the bundle's, its update a repository's. */
  bundled: boolean;
  update: boolean;
  action: RowAction;
}

export interface CatalogFilter {
  query: string;
  kind: KindFilter;
  show: ShowFilter;
}

/** The row of a bundled plugin: its update's offer when a repository has one. */
function bundledRow(plugin: BundledPlugin, names: Map<string, string>): CatalogRow {
  const update = plugin.update;
  const offer: Offer = {
    repository: update?.repository ?? BUNDLE_ORIGIN,
    entry: {
      id: plugin.id,
      name: plugin.name,
      version: update?.version ?? plugin.version,
      kinds: plugin.kinds,
      description: plugin.description,
      capabilities: plugin.grants,
      minAppVersion: null,
      size: plugin.sizeBytes,
      publisher: { keyId: '', fingerprint: '', author: '' },
      releaseNotes: null,
    },
    compatibility: { state: 'compatible' },
  };
  let action: RowAction = 'unchoose';
  if (!plugin.installed) action = 'choose';
  else if (update !== null) action = 'update';
  return {
    key: `${BUNDLE_ORIGIN}/${plugin.id}`,
    offer,
    repositoryName: update === null ? '' : (names.get(update.repository) ?? update.repository),
    severalOrigins: false,
    installedVersion: plugin.installed ? plugin.version : null,
    fromHere: false,
    bundled: true,
    update: plugin.installed && update !== null,
    action,
  };
}

/** The rows of `catalog` that `filter` lets through, by name: the bundled plugins first, then
 * what the repositories offer - the official catalogue's offer of a bundled plugin is its row's
 * update, not a row of its own. */
export function rowsOf(catalog: Catalog, filter: CatalogFilter): CatalogRow[] {
  const names = new Map(catalog.repositories.map((repository) => [repository.id, repository.name]));
  const query = filter.query.trim().toLowerCase();
  const rows: CatalogRow[] = catalog.bundled.map((plugin) => bundledRow(plugin, names));
  const shipped = new Set(catalog.bundled.map((plugin) => plugin.id));
  for (const entry of catalog.entries) {
    for (const offer of entry.offers) {
      if (shipped.has(entry.id) && offer.repository === 'official') continue;
      const install = catalog.installs[entry.id];
      const fromHere = install !== undefined && install.repository === offer.repository;
      const installedVersion = catalog.installed?.[entry.id]?.version ?? install?.version ?? null;
      const update = catalog.updates.some((candidate) => candidate.repository === offer.repository && candidate.entry.id === entry.id);
      const compatible = offer.compatibility.state === 'compatible';
      const bundled = false;
      let action: RowAction = 'none';
      if (update && compatible) action = 'update';
      else if (fromHere) action = 'remove';
      else if (installedVersion === null && compatible) action = 'install';
      rows.push({
        key: `${offer.repository}/${entry.id}`,
        offer,
        repositoryName: names.get(offer.repository) ?? offer.repository,
        severalOrigins: entry.offers.length > 1,
        installedVersion,
        fromHere,
        bundled,
        update,
        action,
      });
    }
  }
  return rows
    .filter((row) => filter.kind === 'all' || row.offer.entry.kinds.includes(filter.kind))
    .filter((row) => filter.show !== 'installed' || row.installedVersion !== null)
    .filter((row) => filter.show !== 'updates' || row.update)
    .filter(
      (row) =>
        query === '' ||
        [row.offer.entry.id, row.offer.entry.name, row.offer.entry.description, row.offer.entry.publisher.author].some((field) =>
          field.toLowerCase().includes(query),
        ),
    )
    .sort((a, b) => a.offer.entry.name.localeCompare(b.offer.entry.name) || a.key.localeCompare(b.key));
}

/** The publisher key of a reviewed package (`catalogPrepare`). */
export interface KeyReview {
  keyId: string;
  fingerprint: string;
  /** This installation trusts the key already. */
  trusted: boolean;
  /** The key of the version the catalogue installed before. */
  previousFingerprint: string | null;
}

/** What the core answers after downloading and verifying a package, before anything is
 * installed or trusted: its full grants and its key (security review H1, M1). */
export interface PackageReview {
  repository: string;
  id: string;
  version: string;
  capabilities: string[];
  grants: PluginGrantDetails;
  key: KeyReview;
}

/** The review the core answered; `null` when it does not read. */
export function parseReview(message: Json): PackageReview | null {
  const review = record(record(record(message).value).review);
  const key = record(review.key);
  const grants = record(review.grants);
  if (typeof review.id !== 'string' || typeof key.fingerprint !== 'string' || typeof grants.evaluate !== 'boolean') {
    return null;
  }
  return {
    repository: text(review.repository),
    id: review.id,
    version: text(review.version),
    capabilities: texts(review.capabilities),
    grants: {
      evaluate: grants.evaluate,
      cdpDomains: texts(grants.cdpDomains),
      ...(grants.windowImage === true ? { windowImage: true } : {}),
      projectGlobs: texts(grants.projectGlobs),
      ...(Array.isArray(grants.writeGlobs) ? { writeGlobs: texts(grants.writeGlobs) } : {}),
      sidecarMethods: texts(grants.sidecarMethods),
      ...(typeof grants.httpRatePerSecond === 'number' ? { httpRatePerSecond: grants.httpRatePerSecond } : {}),
      risky: texts(grants.risky),
    },
    key: {
      keyId: text(key.keyId),
      fingerprint: key.fingerprint,
      trusted: key.trusted === true,
      previousFingerprint: textOrNull(key.previousFingerprint),
    },
  };
}

/** The update is signed by another key than the installed version. */
export function keyChanged(key: KeyReview): boolean {
  return key.previousFingerprint !== null && key.previousFingerprint !== key.fingerprint;
}

/** The key needs its own confirmation step: not trusted yet, or changed. */
export function keyNeedsConfirmation(key: KeyReview): boolean {
  return !key.trusted || keyChanged(key);
}

/** The request of an install the person confirmed: the reviewed repository, id and version, the
 * capabilities and grants the dialog showed, and the key's fingerprint only when the key's own
 * step confirmed it - never as a side effect (security review H1). */
export function installRequest(review: PackageReview, keyConfirmed: boolean) {
  return {
    op: 'catalogInstall' as const,
    repository: review.repository,
    id: review.id,
    version: review.version,
    consent: {
      capabilities: [...review.capabilities],
      grants: review.grants,
      keyFingerprint: keyConfirmed ? review.key.fingerprint : null,
    },
  };
}

/** The i18n key of a capability, as the trust dialog names it. */
export function capabilityKey(capability: string): string {
  return `plugins.capability.${capability}`;
}

/** A failure's text: the core answers a catalogue failure with its i18n key. */
export function failureKey(message: string): string | null {
  return /^plugins\.(catalog\.|bundle\.)?error\.[a-z_]+$/.test(message) ? message : null;
}
