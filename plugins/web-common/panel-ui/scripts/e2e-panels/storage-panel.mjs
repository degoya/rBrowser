// The storage panel in the sandboxed frame (RB-080-84), loaded by e2e.mjs: cookies in two
// sessions and a Firefox view, one JSON value, one cell opened in the editor.
const VIEWS = [
  { id: 1, label: 'Mobile', breakpoint: 'mobile', engine: 'chromium', session: 'default', url: 'https://site.test/', origin: 'https://site.test', status: 'ok' },
  { id: 2, label: 'Tablet', breakpoint: 'tablet', engine: 'chromium', session: 'default', url: 'https://site.test/', origin: 'https://site.test', status: 'ok' },
  { id: 3, label: 'Desktop', breakpoint: 'desktop', engine: 'chromium', session: 'admin', url: 'https://site.test/', origin: 'https://site.test', status: 'ok' },
  { id: 4, label: 'Firefox', breakpoint: 'fx', engine: 'firefox', session: 'default', url: 'https://site.test/', origin: 'https://site.test', status: 'unsupported' },
];
const CART = '{"items":[{"id":7,"qty":2}],"total":19.5}';
const LIST = {
  kind: 'cookies',
  views: VIEWS,
  total: 3,
  shown: 3,
  truncated: false,
  rows: [
    { id: 'c1', scope: 'site.test', path: '/', name: 'consent', state: 'missing', pinned: true, cells: { '1': { value: 'yes', length: 3, domain: 'site.test', path: '/', secure: true }, '2': { value: 'yes', length: 3 }, '3': null } },
    { id: 'c2', scope: 'site.test', path: '/', name: 'cart', state: 'differs', pinned: false, cells: { '1': { value: CART, length: CART.length }, '2': { value: CART, length: CART.length }, '3': { value: '{}', length: 2 } } },
    { id: 'c3', scope: 'site.test', path: '/', name: 'sid', state: 'same', pinned: false, cells: { '1': { value: 'abc123', length: 6, httpOnly: true }, '2': { value: 'abc123', length: 6, httpOnly: true }, '3': { value: 'abc123', length: 6, httpOnly: true } } },
  ],
};
const PINS = [{ id: 'a1b2c3d4e5f60718', kind: 'cookies', origin: 'https://site.test', key: 'consent', value: 'yes', session: null, enabled: true }];

export default {
  title: 'Storage',
  tooltip: '#add',
  answer: (call) => {
    const op = call.args?.op;
    if (op === 'list') return LIST;
    if (op === 'pins') return { pins: PINS };
    if (op === 'value') return { value: CART, length: CART.length };
    return {};
  },
  check: async (frame, expect) => {
    await frame.locator('#grid [data-cell][data-view]').first().waitFor({ timeout: 5000 });
    const states = await frame.locator('#grid [data-cell][data-view]').evaluateAll((all) => all.map((cell) => cell.dataset.state));
    expect('storage-panel: a cell per view, the unread engine and the missing value named', states.slice(0, 4).join(',') === 'value,value,unsupported,missing', states);
    const coloured = await frame.locator('#grid span.value .hl-key').count();
    expect('storage-panel: JSON values coloured', coloured > 0, coloured);
    await frame.locator('#grid [data-cell][data-row="1"][data-col="1"]').click();
    await frame.locator('#editor .hl-editor').waitFor({ timeout: 5000 });
    const backdrop = await frame.locator('#editor .hl-backdrop .hl-key').count();
    expect('storage-panel: the editor colours the value', backdrop > 0, backdrop);
  },
};
