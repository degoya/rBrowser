// The error report panel in the sandboxed frame (RB-080-84), loaded by e2e.mjs: three views (one
// Firefox), an exception in every view with its stack, a 404 only on mobile, a deprecation.
const VIEWS = [
  { id: 1, label: 'Mobile', breakpoint: 'mobile', engine: 'chromium', width: 375, url: 'https://site.test/', captures: true },
  { id: 2, label: 'Desktop', breakpoint: 'desktop', engine: 'chromium', width: 1440, url: 'https://site.test/', captures: true },
  { id: 3, label: 'Firefox', breakpoint: 'tablet', engine: 'firefox', width: 768, url: 'https://site.test/', captures: true },
];

const ANSWER = {
  captured: true,
  total: 3,
  truncated: false,
  summary: { 'js-exception': 1, 'http-not-found': 1, deprecation: 1 },
  severities: { error: 2, warning: 1, info: 0 },
  views: VIEWS,
  entries: [
    {
      id: 'a1', rule: 'js-exception', severity: 'error', text: "Uncaught TypeError: Cannot read properties of undefined (reading 'map')", count: 3,
      views: [1, 2, 3], perView: { 1: 1, 2: 1, 3: 1 }, first: 1, last: 3,
      source: { url: 'https://site.test/assets/app.js?v=7', line: 41, column: 17 },
      stack: "TypeError: Cannot read properties of undefined (reading 'map')\n    at render (https://site.test/assets/app.js:41:17)\n    at https://site.test/assets/app.js:3:1",
    },
    {
      id: 'b2', rule: 'http-not-found', severity: 'error', text: 'GET https://site.test/img/hero-375.png 404', count: 1,
      views: [1], perView: { 1: 1 }, onlyIn: ['mobile'], first: 2, last: 2, url: 'https://site.test/img/hero-375.png',
      request: { method: 'GET', status: 404, resourceType: 'Image' },
    },
    {
      id: 'c3', rule: 'deprecation', severity: 'warning', text: 'Synchronous XMLHttpRequest on the main thread is deprecated.', count: 2,
      views: [1, 2], perView: { 1: 1, 2: 1 }, first: 1, last: 2, source: { url: 'https://site.test/assets/legacy.js', line: 8 },
    },
  ],
};

export default {
  title: 'Error Report',
  tooltip: '#clear',
  answer: (call) => (call.tool === 'get' ? ANSWER : {}),
  check: async (frame, expect) => {
    await frame.locator('li[data-entry]').first().waitFor({ timeout: 5000 });
    const count = await frame.locator('li[data-entry]').count();
    expect('error-report: an entry per error, by category', count === 3, count);
    await frame.locator('li[data-entry]').first().click();
    await frame.locator('[data-stack]').waitFor({ timeout: 5000 });
    const coloured = await frame.locator('[data-stack] [class*="hl-"]').count();
    expect('error-report: the stack opens, coloured', coloured > 0, coloured);
  },
};
