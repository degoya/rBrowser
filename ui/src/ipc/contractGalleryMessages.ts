// The sitemap gallery page's core -> shell message of the contract test's typed literals
// (`contract.test.ts`, RB-090-86), in a file of its own so the core list stays under the
// file-length limit.

import type { CoreToShell } from './protocol';

export const galleryCoreMessages = [
  {
    type: 'sitemapGallery',
    runId: '2026-11-11_10-00-00',
    source: 'http://localhost:3000/',
    state: 'finished',
    thumbnailWidth: 320,
    breakpoints: [{ name: 'mobile', label: 'Mobile', width: 375, height: 812 }],
    pages: [
      {
        url: 'http://localhost:3000/',
        status: 200,
        shots: {
          mobile: {
            state: 'done',
            image: 'rbrowser://sitemap/2026-11-11_10-00-00/index/mobile.png',
            compare: { status: 'differs', ratio: 0.0123 },
          },
        },
      },
      {
        url: 'http://localhost:3000/old',
        status: 404,
        finalUrl: 'http://localhost:3000/new',
        shots: { mobile: { state: 'failed', error: 'timeout' } },
      },
    ],
  },
] satisfies CoreToShell[];
