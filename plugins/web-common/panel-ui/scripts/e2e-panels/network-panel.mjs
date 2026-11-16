// The network panel in the sandboxed frame (RB-080-84), loaded by e2e.mjs: a capture of 2000
// requests in three views (the list draws only the rows in sight), one opened with its JSON body.
const SEEN = { fromCache: false, time: 1, count: 1, durationMs: 42, mimeType: 'application/json' };
const TYPES = ['Document', 'Script', 'Stylesheet', 'Image', 'Fetch'];
const ROWS = Array.from({ length: 2000 }, (_, at) => ({
  method: at % 7 === 0 ? 'POST' : 'GET',
  url: `https://site.test/api/items/${at}?page=${at % 13}`,
  type: TYPES[at % TYPES.length],
  partial: at % 11 === 0,
  differs: at % 17 === 0,
  views: at % 11 === 0
    ? { '1': { ...SEEN, id: `1.${at}`, status: 200, size: 1200 + at } }
    : { '1': { ...SEEN, id: `1.${at}`, status: 200, size: 1200 + at }, '2': { ...SEEN, id: `2.${at}`, status: at % 17 === 0 ? 500 : 200, size: 1200 + at }, '3': { ...SEEN, id: `3.${at}`, status: 200, size: 1200 + at } },
}));
const LIST = {
  rows: ROWS,
  total: 2000,
  captured: true,
  trusted: true,
  rulesRunning: true,
  applied: 0,
  mocksEnabled: true,
  cacheDisabled: false,
  views: [
    { id: 1, label: 'mobile', engine: 'chromium', supported: true, intercepting: true },
    { id: 2, label: 'tablet', engine: 'chromium', supported: true, intercepting: true },
    { id: 3, label: 'desktop', engine: 'firefox', supported: false, intercepting: false },
  ],
};

export default {
  title: 'Network',
  tooltip: '#export',
  answer: (call) => {
    if (call.tool === 'list') return LIST;
    if (call.tool === 'mocks') return { trusted: true, mocksEnabled: true, cacheDisabled: false, dir: '.rbrowser/mocks', files: [], block: [] };
    if (call.tool === 'body') return { text: '{"id": 7, "name": "</pre><b>x</b>", "tags": ["a", "b"]}', base64: false, truncated: false };
    return {};
  },
  check: async (frame, expect) => {
    await frame.locator('#rows [data-row]').first().waitFor({ timeout: 5000 });
    const drawn = await frame.locator('#rows tbody tr').count();
    expect('network-panel: 2000 requests, only the rows in sight drawn', drawn > 0 && drawn < 200, drawn);
    await frame.locator('#rows').evaluate((table) => {
      table.scrollTop = table.scrollHeight;
    });
    await frame.locator('#rows [data-row="1999"]').waitFor({ timeout: 5000 });
    expect('network-panel: scrolled to the end, the last request drawn', true);
    await frame.locator('#rows').evaluate((table) => {
      table.scrollTop = 0;
    });
    await frame.locator('#rows [data-row="0"]').waitFor({ timeout: 5000 });
    await frame.locator('#rows [data-row="0"]').click();
    await frame.locator('#detail-views [data-show-body]').first().click();
    await frame.locator('#body pre .hl-key').first().waitFor({ timeout: 5000 });
    const markup = await frame.locator('#body pre b').count();
    expect('network-panel: the body coloured, its markup text', markup === 0, markup);
  },
};
