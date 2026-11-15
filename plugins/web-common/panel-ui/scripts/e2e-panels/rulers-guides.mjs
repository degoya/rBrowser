// The rulers-guides panel's canned answers for plugins/web-common/panel-ui/scripts/e2e.mjs (PANELS).
export default {
  title: 'Rulers and guides',
  tooltip: '[data-key="remove-0"]',
  answer: (call) =>
    call.tool === 'set'
      ? {
          state: {
            visible: { rulers: true, columns: true, baseline: false },
            guides: [
              { id: 1, axis: 'x', unit: 'percent', value: 50 },
              { id: 2, axis: 'y', unit: 'px', value: 320 },
            ],
            columns: { '*': { preset: 'bootstrap' }, mobile: { columns: 4, gutter: 16, margin: 8 } },
            baseline: { lineHeight: 8, offset: 0 },
          },
          breakpoints: ['desktop', 'mobile'],
          templates: {
            '': { columns: 12, gutter: 24, margin: 0, maxWidth: null },
            bootstrap: { columns: 12, gutter: 24, margin: 12, maxWidth: { min: 540, max: 1320 } },
            tailwind: { columns: 12, gutter: 24, margin: 16, maxWidth: { min: 640, max: 1536 } },
          },
        }
      : {},
};
