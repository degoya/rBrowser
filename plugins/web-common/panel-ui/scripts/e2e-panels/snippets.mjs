// The snippets panel in the sandboxed frame (RB-080-84), loaded by e2e.mjs: three snippets in
// two views, one opened in the coloured editor and run, its results joined.
const LIST = {
  snippets: [
    { source: 'user', name: 'Page title', description: 'The document title', world: 'isolated', target: 'all', timeoutMs: 5000, code: 'return document.title' },
    { source: 'project', name: 'Grid columns', description: 'Columns of the main grid', world: 'main', target: 'all', timeoutMs: 3000, code: 'return getComputedStyle(document.querySelector("main")).gridTemplateColumns' },
    { source: 'local', name: 'Bad header', description: '', world: 'isolated', target: 'all', timeoutMs: 5000, code: '// @world x', error: { code: 'plugin.snippets.errors.header', params: { tag: 'world' } } },
  ],
  project: 'loaded',
  local: 'loaded',
  views: [
    { id: 1, label: 'mobile', breakpoint: 'mobile', engine: 'chromium' },
    { id: 2, label: 'desktop', breakpoint: 'desktop', engine: 'chromium' },
  ],
  active: 2,
};

export default {
  title: 'Snippets',
  tooltip: '#new',
  answer: (call) => {
    if (call.tool === 'list') return LIST;
    if (call.tool === 'run') {
      const view = call.args.target;
      return view === 1
        ? { views: [1], results: [{ ok: false, type: 'error', text: 'TypeError: x is not a function', stack: 'TypeError: x is not a function\n    at f (https://site.test/app.js:3:7)', views: [1] }] }
        : { views: [2], results: [{ ok: true, type: 'object', text: '{"title": "Home", "items": [1, 2, 3]}', views: [2] }] };
    }
    return {};
  },
  check: async (frame, expect) => {
    await frame.locator('#list [role="option"]').first().click();
    const coloured = await frame.locator('.hl-editor .hl-backdrop .hl-keyword').count();
    expect('snippets: the editor colours the code', coloured > 0, coloured);
    await frame.locator('#run').click();
    await frame.locator('#results [data-outcome]').nth(1).waitFor({ timeout: 5000 });
    const keys = await frame.locator('#results [data-text] .hl-key').count();
    expect('snippets: a run in two views, its value coloured', keys > 0, keys);
    await frame.locator('#results [data-stack-toggle]').click();
    await frame.locator('#results [data-stack]').waitFor({ timeout: 5000 });
  },
};
