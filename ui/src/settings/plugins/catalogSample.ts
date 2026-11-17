// A catalogue answer as the core sends it (`plugins/catalog.rs`, RB-091-01), for the tests: the
// official catalogue and a person's repository, the same plugin from both, an update, a plugin
// the catalogue installed, one to install and one that needs a newer rBrowser.

function entry(id: string, name: string, version: string, extra: Record<string, unknown> = {}) {
  return {
    id,
    name,
    version,
    api_version: '0.1.0',
    kinds: ['feature-plugin'],
    description: `${name} for rBrowser.`,
    capabilities: ['views'],
    url: `${id}-${version}.rbplugin`,
    sha256: '0'.repeat(64),
    size: 4096,
    publisher: { key_id: 'rbrowser-plugin-v1', fingerprint: 'ab'.repeat(32), author: 'degoya' },
    ...extra,
  };
}

const compatible = { state: 'compatible' };
const seoOfficial = { repository: 'official', entry: entry('seo', 'SEO', '1.1.0', { kinds: ['check-plugin'] }), compatibility: compatible };

export const SAMPLE_CATALOG = {
  repositories: [
    { id: 'official', name: 'rBrowser', url: 'https://example.test/rbrowser-plugin-index.json', official: true, fingerprint: null, loaded: true, error: null },
    { id: 'user-0123456789abcdef', name: 'Mine', url: 'https://mine.test/rbrowser-plugin-index.json', official: false, fingerprint: 'cd'.repeat(32), loaded: true, error: null },
  ],
  refused: [],
  entries: [
    { id: 'future', offers: [{ repository: 'official', entry: entry('future', 'Future', '2.0.0'), compatibility: { state: 'needsNewerApp', minAppVersion: '9.0.0' } }] },
    { id: 'outline', offers: [{ repository: 'official', entry: entry('outline', 'Outline', '1.0.0'), compatibility: compatible }] },
    {
      id: 'seo',
      offers: [
        seoOfficial,
        { repository: 'user-0123456789abcdef', entry: entry('seo', 'SEO', '2.0.0', { kinds: ['check-plugin'] }), compatibility: compatible },
      ],
    },
    { id: 'overlays', offers: [{ repository: 'official', entry: entry('overlays', 'Overlays', '2.0.0'), compatibility: compatible }] },
    {
      id: 'todo',
      offers: [
        {
          repository: 'official',
          entry: entry('todo', 'Todo', '1.0.0', { capabilities: ['views', 'out-write'], description: 'Tasks in the project.' }),
          compatibility: compatible,
        },
      ],
    },
  ],
  updates: [seoOfficial],
  installs: {
    outline: { repository: 'official', version: '1.0.0', sha256: '0'.repeat(64) },
    seo: { repository: 'official', version: '1.0.0', sha256: '0'.repeat(64) },
  },
  installed: {
    overlays: { version: '1.0.0', source: 'bundled' },
    outline: { version: '1.0.0', source: 'user' },
    seo: { version: '1.0.0', source: 'user' },
  },
};

/** The core's review of `todo` (`catalogPrepare`): its full grants and an untrusted key. */
export const SAMPLE_REVIEW = {
  value: {
    review: {
      repository: 'official',
      id: 'todo',
      version: '1.0.0',
      capabilities: ['views', 'cdp'],
      grants: { evaluate: false, cdpDomains: ['DOM'], projectGlobs: [], sidecarMethods: [], risky: ['cdp:DOM'] },
      key: { keyId: 'rbrowser-plugin-v1', fingerprint: 'ab'.repeat(32), trusted: false, previousFingerprint: null },
    },
  },
};

/** The same review for an update whose key changed. */
export const SAMPLE_CHANGED_KEY_REVIEW = {
  value: {
    review: {
      ...SAMPLE_REVIEW.value.review,
      key: { keyId: 'rbrowser-plugin-v1', fingerprint: 'ab'.repeat(32), trusted: true, previousFingerprint: 'cd'.repeat(32) },
    },
  },
};
