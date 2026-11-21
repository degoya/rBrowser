// A run for the gallery page's tests (RB-090-86): two breakpoints, a fine page, a 404 with a
// redirect and a failed shot.
import type { SitemapGalleryPage } from '../../ipc/protocol';

export const RUN_ID = '2026-11-11_10-00-00';

const image = (slug: string, breakpoint: string): string => `rbrowser://sitemap/${RUN_ID}/${slug}/${breakpoint}.png`;

export function galleryRun(): SitemapGalleryPage {
  return {
    runId: RUN_ID,
    source: 'http://localhost:3000/',
    state: 'finished',
    thumbnailWidth: 320,
    breakpoints: [
      { name: 'mobile', label: 'Mobile', width: 375, height: 812 },
      { name: 'desktop', label: '', width: 1440, height: 900 },
    ],
    pages: [
      {
        url: 'http://localhost:3000/',
        status: 200,
        shots: {
          mobile: { state: 'done', image: image('index', 'mobile') },
          desktop: { state: 'done', image: image('index', 'desktop'), compare: { status: 'differs', ratio: 0.0123 } },
        },
      },
      {
        url: 'http://localhost:3000/missing',
        status: 404,
        shots: { mobile: { state: 'done', image: image('missing', 'mobile') }, desktop: { state: 'pending' } },
      },
      {
        url: 'http://localhost:3000/old',
        status: 200,
        finalUrl: 'http://localhost:3000/new',
        shots: { mobile: { state: 'failed', error: 'timeout' }, desktop: { state: 'done', image: image('old', 'desktop') } },
      },
    ],
  };
}
