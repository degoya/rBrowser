// Milestone 0.8, capture (M9, docs/roadmap.md "0.8"): screenshots of every view without cookie
// banners and with the animations frozen, the screenshot editor plugin loaded, and the video
// recordings listed (recording itself downloads ffmpeg on demand: a documented measurement,
// tests/e2e/README.md).

import { measure, outcome } from './lib/case.ts';
import type { Case } from './lib/case.ts';
import { needSession, site } from './lib/context.ts';

export const m09: Case = {
  id: 'M9',
  title: 'Milestone 0.8 capture',
  threshold: 'a screenshot per view without banners; screenshot-editor active; video_list answers',
  project: 'e2e',
  needs: [],
  jobs: ['RB-080-01', 'RB-080-02', 'RB-080-12'],
  async run(ctx) {
    const session = needSession(ctx);
    await session.navigateAll(site(ctx, '/grid.html'));
    const views = (await session.views()).length;
    const shot = await session.mcp.call(
      'screenshot',
      { target: 'all', hideBanners: true, freezeAnimations: true, save: true },
      120_000,
    );
    const shots = Array.isArray(shot.value) ? shot.value.length : (shot.value as { views?: unknown[] } | null)?.views?.length ?? 0;
    const list = await session.mcp.value<{ plugins?: { id?: string; state?: string }[] }>('plugins_list');
    const editor = list.plugins?.find((plugin) => plugin.id === 'screenshot-editor')?.state ?? 'not listed';
    const videos = await session.mcp.call('video_list');
    return outcome([
      measure('screenshots', shot.isError ? 'error' : `${shots} for ${views} views`, 'one per view', !shot.isError && shots === views),
      measure('screenshot-editor', editor, 'active', editor === 'active'),
      measure('video_list', videos.isError ? 'error' : 'answered', 'answered', !videos.isError),
    ]);
  },
};
