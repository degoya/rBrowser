// The image-overlay panel's canned answers for plugins/web-common/panel-ui/scripts/e2e.mjs (PANELS).
const PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==';
const STATE = {
  enabled: true, opacity: 50, blend: 'difference', invert: false, anchor: 'document',
  offset: { x: 0, y: 0 }, split: { on: false, position: 50 }, autoScaleDpr: true,
  designs: { mobile: 'designs/mobile.png' },
};
const MOBILE = { name: 'mobile.png', path: 'designs/mobile.png', mime: 'image/png', bytes: 70, hash: 'bbbbbbbbbbbbbbbb', width: 393, height: 852, scale: 1, chunks: 1 };
const DESKTOP = { name: 'desktop-1440.png', path: 'designs/desktop-1440.png', mime: 'image/png', bytes: 70, hash: 'aaaaaaaaaaaaaaaa', width: 1440, height: 900, scale: 1, chunks: 1 };

export default {
  title: 'Image overlay',
  tooltip: '#add-design',
  answer: (call) => {
    if (call.tool === 'set') {
      return {
        state: STATE,
        views: [
          { view: 1, label: 'Mobile', breakpoint: 'mobile', image: { path: 'designs/mobile.png', width: 393, height: 852, scale: 1 }, problem: null },
          { view: 2, label: 'Desktop', breakpoint: 'desktop', image: { path: 'designs/desktop-1440.png', width: 1440, height: 900, scale: 1 }, problem: null },
        ],
        designs: ['desktop-1440.png', 'mobile.png'],
      };
    }
    if (call.tool === 'designs') {
      if (call.args.thumb) return { thumb: call.args.thumb, data: call.args.data ? undefined : PNG, saved: Boolean(call.args.data) };
      if (call.args.path) return { index: call.args.chunk, data: PNG };
      return { designs: [DESKTOP, MOBILE], more: false, trusted: true, chunkBytes: 48128 };
    }
    return {};
  },
};
