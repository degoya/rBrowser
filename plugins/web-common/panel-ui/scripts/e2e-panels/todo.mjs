// The todo panel in the sandboxed frame (RB-110-13), loaded by e2e.mjs: one file with a project,
// a todo, a done one and the archive; a check sends `toggle` with the version shown.
const FILE = {
  path: 'TODO.todo',
  rev: '0123456789abcdef',
  counts: { open: 1, done: 1, cancelled: 0 },
  items: [
    { line: 1, depth: 0, kind: 'project', text: 'Website', raw: 'Website:', tags: [], project: '', archived: false },
    { line: 2, depth: 1, kind: 'task', state: 'open', symbol: '☐', text: 'Landing page', raw: 'Landing page @high', tags: [{ name: 'high', value: null }], project: 'Website', archived: false },
    { line: 3, depth: 1, kind: 'task', state: 'done', symbol: '✔', text: 'Colours', raw: 'Colours @done(26-11-14 09:30)', tags: [{ name: 'done', value: '26-11-14 09:30' }], project: 'Website', archived: false },
    { line: 5, depth: 0, kind: 'project', text: 'Archive', raw: 'Archive:', tags: [], project: '', archived: false },
    { line: 6, depth: 1, kind: 'task', state: 'done', symbol: '✔', text: 'Beta', raw: 'Beta @done(26-10-01 12:00) @project(Website)', tags: [], project: 'Archive', archived: true },
  ],
};

let toggled = null;

export default {
  title: 'Todo',
  tooltip: '#reload',
  answer: (call) => {
    if (call.tool === 'list') return { files: [FILE], truncated: false };
    if (call.tool === 'toggle') {
      toggled = call.args;
      const items = FILE.items.map((item) => (item.line === 2 ? { ...item, state: 'done' } : item));
      return { line: 2, state: 'done', file: { ...FILE, rev: 'fedcba9876543210', items } };
    }
    return {};
  },
  check: async (frame, expect) => {
    await frame.locator('[data-line="2"] [data-check]').click();
    await frame.locator('[data-line="2"][data-state="done"]').waitFor({ timeout: 5000 });
    const sent = toggled !== null && toggled.rev === FILE.rev && toggled.line === 2;
    expect('todo: a check sends toggle with the version shown', sent, toggled);
  },
};
