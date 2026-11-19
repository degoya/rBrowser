// Settings > Plugins > Catalog, the bundled plugins (RB-091-04): one row each with its origin,
// a one-click install of what is not chosen, the update a repository offers, and no second row
// of the official catalogue for the same plugin.
import { describe, expect, it } from 'vitest';
import { type CatalogFilter, parseCatalog, rowsOf } from './catalog';
import { SAMPLE_CATALOG } from './catalogSample';

const all: CatalogFilter = { query: '', kind: 'all', show: 'all' };

const withBundle = {
  ...SAMPLE_CATALOG,
  bundled: [
    {
      id: 'overlays',
      name: 'Overlays',
      description: 'Debug overlays.',
      version: '1.0.0',
      kinds: ['feature-plugin'],
      category: 'overlays',
      sizeBytes: 2048,
      recommended: true,
      grants: ['views'],
      installed: true,
      origin: 'bundle',
      update: { repository: 'official', version: '2.0.0' },
    },
    {
      id: 'peek',
      name: 'Peek',
      description: 'Hold Alt.',
      version: '1.0.0',
      kinds: ['feature-plugin'],
      category: 'overlays',
      sizeBytes: 1024,
      recommended: false,
      grants: [],
      installed: false,
      origin: 'bundle',
      update: null,
    },
    {
      id: 'fonts',
      name: 'Fonts',
      description: 'Fonts.',
      version: '1.0.0',
      kinds: ['check-plugin'],
      category: 'checks',
      sizeBytes: 1024,
      recommended: false,
      grants: [],
      installed: true,
      origin: 'bundle',
      update: null,
    },
    { id: 3, name: null },
  ],
};

describe('the bundled plugins in the catalogue', () => {
  it('reads the bundled rows and drops what is malformed', () => {
    const catalog = parseCatalog(withBundle);
    expect(catalog.bundled.map((plugin) => plugin.id)).toEqual(['overlays', 'peek', 'fonts']);
    expect(catalog.bundled[0]?.update).toEqual({ repository: 'official', version: '2.0.0' });
    expect(parseCatalog(SAMPLE_CATALOG).bundled).toEqual([]);
  });

  it('offers a one-click install, the update of a repository, or taking it out', () => {
    const rows = rowsOf(parseCatalog(withBundle), all);
    const row = (id: string) => rows.find((candidate) => candidate.offer.entry.id === id && candidate.bundled);
    expect(row('peek')?.action).toBe('choose');
    expect(row('fonts')?.action).toBe('unchoose');
    const overlays = row('overlays');
    expect(overlays?.action).toBe('update');
    // The update is the repository's offer: the review downloads that one.
    expect(overlays?.offer.repository).toBe('official');
    expect(overlays?.offer.entry.version).toBe('2.0.0');
    expect(overlays?.installedVersion).toBe('1.0.0');
  });

  it('lists a bundled plugin once, not again as the official catalogue offers it', () => {
    const rows = rowsOf(parseCatalog(withBundle), all).filter((row) => row.offer.entry.id === 'overlays');
    expect(rows).toHaveLength(1);
    expect(rows[0]?.bundled).toBe(true);
  });

  it('filters the bundled rows like the others', () => {
    const catalog = parseCatalog(withBundle);
    const ids = (filter: Partial<CatalogFilter>) =>
      rowsOf(catalog, { ...all, ...filter })
        .filter((row) => row.bundled)
        .map((row) => row.offer.entry.id);
    expect(ids({ kind: 'check-plugin' })).toEqual(['fonts']);
    expect(ids({ show: 'installed' })).toEqual(['fonts', 'overlays']);
    expect(ids({ show: 'updates' })).toEqual(['overlays']);
    expect(ids({ query: 'alt' })).toEqual(['peek']);
  });
});

describe('the answer of a one-click install', () => {
  it('names the first failure, or none', async () => {
    const { chosenFailure } = await import('./bundledRows');
    expect(chosenFailure({ value: { results: [{ id: 'peek', ok: true, origin: 'bundle' }], restart: true } })).toBeNull();
    expect(chosenFailure({ value: { results: [{ id: 'peek', ok: false, error: 'plugins.bundle.error.offline' }] } })).toBe(
      'plugins.bundle.error.offline',
    );
    expect(chosenFailure({ value: { results: [{ id: 'peek' }] } })).toBe('plugins.bundle.error.unknown');
    expect(chosenFailure(null)).toBeNull();
  });
});
