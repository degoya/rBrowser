// The simulators picker (RB-080-86) for plugins/web-common/panel-ui/scripts/e2e.mjs: its PANELS entry.
export default {
  title: 'Simulators',
  tooltip: '#refresh',
  answer: (call) =>
    call.tool === 'set'
      ? {
          active: 2,
          views: [
            { view: 1, label: 'Desktop', engine: 'chromium', type: 'none', severity: null, mode: 'off' },
            { view: 2, label: 'Mobile', engine: 'chromium', type: 'protanomaly', severity: 0.4, mode: 'script' },
          ],
          catalog: [
            { id: 'protanomaly', group: 'color', severity: 0.6, native: false },
            { id: 'deuteranopia', group: 'color', severity: null, native: true },
            { id: 'myopia', group: 'vision', severity: 1, native: true },
            { id: 'dimmed', group: 'environment', severity: 0.6, native: false },
          ],
        }
      : {},
};
