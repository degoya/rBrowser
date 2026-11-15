// The Outline panel in panel-ui's e2e (RB-080-85): the headings of two views, one entry only in
// one of them; a click marks it, which enables Clear marks.
const cell = (path) => ({ index: 0, visible: true, hidden: false, selector: `#${path}`, path });

export default {
  title: 'Outline',
  tooltip: '#refresh',
  answer: (call) => {
    if (call.tool === 'mark') return call.args.clear ? {} : { marked: call.args.targets.map(() => ({ found: true })) };
    if (call.tool !== 'get') return {};
    return {
      views: [
        { id: 1, label: 'Desktop' },
        { id: 2, label: 'Phone' },
      ],
      findings: [],
      sections: {
        headings: {
          rows: [
            { item: { level: 1, text: 'Title' }, views: { 1: cell('h1'), 2: cell('h1') } },
            { item: { level: 2, text: 'Only wide' }, views: { 1: cell('h2') }, only: [1] },
          ],
        },
      },
    };
  },
  /** The sections are tabs; a click on an entry marks it and enables Clear marks. */
  check: async (frame, expect) => {
    const tabs = await frame.locator('#tabs [role="tab"]').count();
    expect('outline: the sections are tabs', tabs === 8, tabs);
    await frame.locator('#tree [role="treeitem"]').first().click();
    await frame.locator('#clear:not([disabled])').waitFor({ timeout: 5000 });
    expect('outline: marking enables Clear marks', true);
  },
};
