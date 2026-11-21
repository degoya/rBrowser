// Settings > Plugins > Catalog (RB-091-01): the core's answer read defensively, one row per
// plugin and origin, the filters, and the install request carrying what the dialog showed.
import { describe, expect, it } from 'vitest';
import {
  type CatalogFilter,
  failureKey,
  installRequest,
  keyChanged,
  keyNeedsConfirmation,
  parseCatalog,
  parseReview,
  restartAsked,
  rowsOf,
} from './catalog';
import { SAMPLE_CATALOG, SAMPLE_REVIEW } from './catalogSample';

const all: CatalogFilter = { query: '', kind: 'all', show: 'all' };

describe('the plugin catalogue', () => {
  it('reads the answer and drops what is malformed', () => {
    const catalog = parseCatalog(SAMPLE_CATALOG);
    expect(catalog.repositories.map((repository) => repository.id)).toEqual(['official', 'user-0123456789abcdef']);
    expect(catalog.entries[0]?.offers[0]?.entry.publisher.keyId).toBe('rbrowser-plugin-v1');
    expect(catalog.installed?.['seo']?.source).toBe('user');
    const broken = parseCatalog({ repositories: 'no', entries: [{ id: 3, offers: [{ entry: null }] }], installed: null });
    expect(broken.repositories).toEqual([]);
    expect(broken.entries[0]?.offers[0]?.entry.capabilities).toEqual([]);
    expect(broken.installed).toBeNull();
    expect(parseCatalog(null).entries).toEqual([]);
  });

  it('lists the same id from two repositories as two rows naming their origin', () => {
    const rows = rowsOf(parseCatalog(SAMPLE_CATALOG), all).filter((row) => row.offer.entry.id === 'seo');
    expect(rows.map((row) => [row.offer.repository, row.repositoryName, row.severalOrigins])).toEqual([
      ['official', 'rBrowser', true],
      ['user-0123456789abcdef', 'Mine', true],
    ]);
    // Installed from the official catalogue: the update comes from there only.
    expect(rows.map((row) => row.action)).toEqual(['update', 'none']);
  });

  it('offers install, remove or nothing as the row stands', () => {
    const rows = rowsOf(parseCatalog(SAMPLE_CATALOG), all);
    const action = (id: string) => rows.find((row) => row.offer.entry.id === id)?.action;
    expect(action('todo')).toBe('install');
    expect(action('outline')).toBe('remove');
    expect(action('future')).toBe('none');
  });

  it('shows a plugin the start loaded as installed; its bundled row comes from `bundled` (RB-091-04)', () => {
    const row = rowsOf(parseCatalog(SAMPLE_CATALOG), all).find((candidate) => candidate.offer.entry.id === 'overlays');
    expect(row?.bundled).toBe(false);
    expect(row?.installedVersion).toBe('1.0.0');
    expect(row?.action).toBe('none');
  });

  it('filters by text, kind, installed and updates', () => {
    const catalog = parseCatalog(SAMPLE_CATALOG);
    const ids = (filter: Partial<CatalogFilter>) => [...new Set(rowsOf(catalog, { ...all, ...filter }).map((row) => row.offer.entry.id))];
    expect(ids({ query: 'TASK' })).toEqual(['todo']);
    expect(ids({ kind: 'check-plugin' })).toEqual(['seo']);
    expect(ids({ show: 'installed' })).toEqual(['outline', 'overlays', 'seo']);
    expect(ids({ show: 'updates' })).toEqual(['seo']);
  });

  it('reads a review and asks for exactly what it showed, the key only when its own step confirmed it', () => {
    const review = parseReview(SAMPLE_REVIEW);
    if (review === null) throw new Error('the sample review reads');
    expect(review.grants.cdpDomains).toEqual(['DOM']);
    expect(keyNeedsConfirmation(review.key)).toBe(true);
    expect(installRequest(review, false)).toEqual({
      op: 'catalogInstall',
      repository: 'official',
      id: 'todo',
      version: '1.0.0',
      consent: { capabilities: ['views', 'cdp'], grants: review.grants, keyFingerprint: null },
    });
    expect(installRequest(review, true).consent.keyFingerprint).toBe('ab'.repeat(32));
    expect(parseReview({ value: { review: { id: 'x' } } })).toBeNull();
  });

  it('tells a changed key from a trusted one', () => {
    const key = { keyId: 'k', fingerprint: 'ab'.repeat(32), trusted: true, previousFingerprint: null };
    expect(keyNeedsConfirmation(key)).toBe(false);
    expect(keyChanged({ ...key, previousFingerprint: 'cd'.repeat(32) })).toBe(true);
    expect(keyNeedsConfirmation({ ...key, previousFingerprint: 'cd'.repeat(32) })).toBe(true);
  });

  it('translates only the failure codes of the plugins', () => {
    expect(failureKey('plugins.catalog.error.checksum')).toBe('plugins.catalog.error.checksum');
    expect(failureKey('plugins.error.signature_invalid')).toBe('plugins.error.signature_invalid');
    expect(failureKey('plugins.bundle.error.offline')).toBe('plugins.bundle.error.offline');
    expect(failureKey('the window is closed')).toBeNull();
  });

  it('says "at the next start" only when the core could not load the change live (RB-092-06)', () => {
    expect(restartAsked({ value: { id: 'seo', version: '1.0.0', restart: false } })).toBe(false);
    expect(restartAsked({ value: { id: 'seo', restart: true } })).toBe(true);
    expect(restartAsked({ value: { results: [], restart: true } })).toBe(true);
    // An answer without the flag is no reason to ask for a restart.
    expect(restartAsked({ value: { id: 'seo' } })).toBe(false);
    expect(restartAsked(null)).toBe(false);
    expect(restartAsked({ value: 'restart' })).toBe(false);
  });
});
