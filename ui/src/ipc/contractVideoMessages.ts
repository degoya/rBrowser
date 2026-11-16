// The Videos panel's core -> shell messages of the contract test's typed literals
// (`contract.test.ts`, RB-110-12), in a file of their own so the test stays under the
// file-length limit.

import type { CoreToShell } from './protocol';

export const videoMessages = [
  {
    type: 'videos',
    project: true,
    folder: 'C:/work/shop/.rbrowser/out/video',
    autoplay: false,
    recordings: [
      {
        id: '2026-11-16_14-03-07',
        path: 'C:/work/shop/.rbrowser/out/video/2026-11-16_14-03-07',
        files: [
          {
            name: 'mobile.mp4',
            breakpoint: 'mobile',
            bytes: 524288,
            path: 'C:/work/shop/.rbrowser/out/video/2026-11-16_14-03-07/mobile.mp4',
            url: 'rbrowser://video/2026-11-16_14-03-07/mobile.mp4',
            poster: 'rbrowser://video/2026-11-16_14-03-07/mobile.jpg',
          },
        ],
      },
    ],
    mp4Export: true,
    conversions: [
      {
        recording: '2026-11-16_14-03-07',
        file: 'mobile.mp4',
        target: 'mobile.webm',
        to: 'webm',
        running: true,
        percent: 40,
      },
    ],
  },
  { type: 'videosChanged' },
] satisfies CoreToShell[];
