// The overlays panel (RB-080-86) for plugins/web-common/panel-ui/scripts/e2e.mjs: its PANELS entry.
export default {
  title: 'Overlays',
  tooltip: '#off',
  answer: (call) =>
    call.tool === 'state'
      ? { overlays: { overflow: { enabled: true }, highlight: { enabled: true, options: { selector: '.card' } } } }
      : call.command === 'highlight'
        ? { matches: [{ count: 2 }] }
        : {},
};
