// The console panel in the sandboxed frame (RB-080-84), loaded by e2e.mjs: 3000 messages in three
// views (the list draws only the rows in sight), one with a stack opened, a JSON message coloured.
const LEVELS = ['error', 'warning', 'info', 'log', 'network'];
const ENTRIES = Array.from({ length: 3000 }, (_, at) => ({
  id: 'e' + at,
  level: LEVELS[at % LEVELS.length],
  kind: at % 5 === 0 ? 'exception' : 'console',
  text: at === 1 ? '{"width": 320, "name": "</pre><b>x</b>"}' : `message ${at}: layout at ${320 + at} px`,
  count: (at % 4) + 1,
  views: at % 3 === 0 ? [1, 2] : [1, 2, 3],
  perView: { '1': 1, '2': 1, '3': 1 },
  first: at,
  last: at,
  source: { url: `https://site.test/assets/app.js?v=${at % 9}`, line: at % 200 },
  stack: at % 5 === 0 ? `TypeError: x is not a function\n    at f (https://site.test/assets/app.js:${at}:5)` : undefined,
}));
const GROUPED = {
  captured: true,
  total: 3000,
  shown: 3000,
  truncated: false,
  levels: { error: 600, warning: 600, info: 600, log: 600, debug: 0, network: 600 },
  views: [
    { id: 1, label: 'mobile', breakpoint: 'mobile', engine: 'chromium', width: 375 },
    { id: 2, label: 'tablet', breakpoint: 'tablet', engine: 'firefox', width: 768 },
    { id: 3, label: 'desktop', breakpoint: 'desktop', engine: 'webkit', width: 1280 },
  ],
  entries: ENTRIES,
};

export default {
  title: 'Console',
  tooltip: '#clear',
  answer: (call) => (call.tool === 'grouped' ? GROUPED : {}),
  check: async (frame, expect) => {
    await frame.locator('#entries [data-entry]').first().waitFor({ timeout: 5000 });
    const drawn = await frame.locator('#entries tbody tr').count();
    expect(`console: 3000 messages, only the rows in sight drawn (${drawn} rows)`, drawn > 0 && drawn < 200, drawn);
    await frame.locator('#entries').evaluate((table) => {
      table.scrollTop = table.scrollHeight;
    });
    await frame.locator('#entries [data-entry="2999"]').waitFor({ timeout: 5000 });
    expect('console: scrolled to the end, the last message drawn', true);
    await frame.locator('#entries').evaluate((table) => {
      table.scrollTop = 0;
    });
    await frame.locator('#entries [data-entry="0"]').waitFor({ timeout: 5000 });
    const json = await frame.locator('#entries [data-entry="1"] .hl-key').count();
    const markup = await frame.locator('#entries [data-entry="1"] b').count();
    expect('console: a JSON message coloured, its markup text', json > 0 && markup === 0, { json, markup });
    await frame.locator('#entries [data-entry="0"]').click();
    await frame.locator('#entries .stack .hl-keyword').first().waitFor({ timeout: 5000 });
    expect('console: a stack opens, coloured', true);
  },
};
