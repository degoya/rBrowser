// The sitemap start page's messages of the contract test's typed literals (`contract.test.ts`,
// RB-099-09), in a file of its own so the core list stays under the file-length limit.

import type { CoreToShell, ShellToCore } from './protocol';

export const sitemapStartShellMessages = [{ type: 'sitemapStart', op: 'discover' }] satisfies ShellToCore[];

export const sitemapStartCoreMessages = [
  {
    type: 'sitemapDiscovery',
    discoveryId: '1795000000-1',
    source: 'http://localhost:3000/',
    site: 'http://localhost:3000',
    pages: ['http://localhost:3000/', 'http://localhost:3000/de'],
    tree: [
      {
        name: 'http://localhost:3000',
        first: 0,
        count: 2,
        children: [
          { name: '/', first: 0, count: 1 },
          { name: 'de', first: 1, count: 1 },
        ],
      },
    ],
    truncated: false,
    maxPages: 100000,
    views: { all: 4, visible: 2 },
  },
] satisfies CoreToShell[];
