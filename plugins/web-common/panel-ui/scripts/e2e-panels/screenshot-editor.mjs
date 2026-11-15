// The screenshot editor panel in e2e.mjs (RB-080-86): the
// list, a real 1 x 1 PNG for `load` (one chunk), `annotate` answers a path. Its check: the image
// is on the canvas and the annotation font (a data URL, panel/annotation-font.css) loaded under
// the panel CSP.
const PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==';
const SHOT = '.rbrowser/out/screenshots/2026-11-15_10-00-00/mobile.png';

export default {
  title: 'Screenshot editor',
  tooltip: '#undo',
  answer: (call) => {
    if (call.tool === 'list') return { images: [SHOT, '.rbrowser/out/checks/run/diff.png'], truncated: false };
    if (call.tool === 'load') return { image: SHOT, bytes: 68, offset: 0, next: null, hash: 'h1', data: PNG, width: 1, height: 1, annotations: [] };
    if (call.tool === 'annotate') return { path: '.rbrowser/out/screenshot-editor/screenshots/2026-11-15_10-00-00/mobile.edited.png' };
    return {};
  },
  check: async (frame) => {
    await frame.locator('#status').filter({ hasText: '1 x 1 px' }).waitFor({ timeout: 5000 });
    const font = await frame.locator('body').evaluate(async () => {
      await document.fonts.load('20px ShotAnnotation');
      return document.fonts.check('20px ShotAnnotation') && [...document.fonts].some((face) => face.family.replace(/["']/g, '') === 'ShotAnnotation' && face.status === 'loaded');
    });
    if (!font) throw new Error('the annotation font did not load');
  },
};
