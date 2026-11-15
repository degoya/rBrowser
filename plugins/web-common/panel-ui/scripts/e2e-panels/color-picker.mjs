// The colour picker panel (RB-080-86) for e2e.mjs: `contrast` answers a rated pair, `palette`
// two colours of one view, `measure_text` two views. Its check: the formats and levels are drawn,
// and reading the palette lists its colours.
const formats = (hex, rgb) => ({ hex, rgb, hsl: 'hsl(0 0% 46%)', oklch: 'oklch(0.56 0 0)' });

export default {
  title: 'Colour picker',
  tooltip: '#swap',
  answer: (call) => {
    if (call.tool === 'contrast') {
      return { ratio: 4.54, normal: { aa: true, aaa: false }, large: { aa: true, aaa: true }, foreground: formats('#767676', 'rgb(118, 118, 118)'), background: formats('#ffffff', 'rgb(255, 255, 255)') };
    }
    if (call.tool === 'palette') {
      return { views: [{ view: 1, label: 'md', colors: [{ hex: '#212121', count: 7, members: ['rgb(33, 33, 33)'] }, { hex: '#00796b', count: 1, members: ['rgb(0, 121, 107)'] }] }] };
    }
    if (call.tool === 'measure_text') {
      return { selector: 'p', optimal: { min: 45, max: 75 }, warnAbove: 80, views: [{ view: 1, label: 'md', lines: 12, average: 64, max: 78, over: 0, rating: 'optimal' }, { view: 2, label: 'mobile', error: 'plugin.color-picker.errors.unsupported_engine' }] };
    }
    return {};
  },
  check: async (frame, expect) => {
    await frame.locator('[data-slot="foreground"] [data-formats] li').nth(3).waitFor({ timeout: 5000 });
    await frame.locator('#palette-read').click();
    await frame.locator('#palette li').nth(1).waitFor({ timeout: 5000 });
    await frame.locator('#measure-run').click();
    await frame.locator('#measure-table tbody tr').nth(1).waitFor({ timeout: 5000 });
    const ratio = await frame.locator('#ratio').textContent();
    expect('color-picker: the ratio is drawn', ratio === '4.54:1', ratio);
  },
};
