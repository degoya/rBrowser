import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import SettingsPage from './SettingsPage.vue';
import { createFakeCore, type FakeCore } from './test/fakeCore';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

async function open(props: Record<string, unknown> = {}, core = createFakeCore()) {
  const mounted = await mountPage(SettingsPage, { routeKey: null, ...props }, core);
  current = mounted.wrapper;
  return mounted;
}

afterEach(() => {
  current?.unmount();
  current = null;
});

function tabs(wrapper: VueWrapper) {
  return wrapper.findAll('[data-testid="settings-tabs"] button[role="tab"]');
}

function selectedTab(wrapper: VueWrapper): string | undefined {
  return tabs(wrapper)
    .find((tab) => tab.attributes('aria-selected') === 'true')
    ?.text();
}

function sections(wrapper: VueWrapper): (string | undefined)[] {
  return wrapper.findAll('[data-section]').map((section) => section.attributes('data-section'));
}

describe('the sub-tabs of Layout and Breakpoints & Devices (RB-080-13)', () => {
  it('splits Layout into its seven sections, one card at a time, and puts the tab in the address', async () => {
    const { wrapper } = await open({ routeKey: 'layout', routeTab: 'presets' });
    expect(tabs(wrapper).map((tab) => tab.text())).toEqual([
      'Layout presets',
      'Grid',
      'Zoom',
      'Focus mode',
      'Further modes',
      'Status bar',
      'Pop-out',
    ]);
    expect(selectedTab(wrapper)).toBe('Layout presets');
    expect(sections(wrapper)).toEqual(['layout.presets']);
    await tabs(wrapper)[1]!.trigger('mousedown', { button: 0 });
    await settle();
    expect(sections(wrapper)).toEqual(['layout.grid']);
    expect(window.location.hash).toBe('#/settings/layout?tab=grid');
  });

  it('keeps the tabs in one row that scrolls in a narrow window, never wrapped (RB-080-81)', async () => {
    const { wrapper } = await open({ routeKey: 'layout', routeTab: 'presets' });
    const list = wrapper.get('[data-testid="settings-tabs"] [role="tablist"]');
    expect(list.classes()).toContain('overflow-x-auto');
    expect(list.classes()).not.toContain('flex-wrap');
  });

  it('switches tabs with the arrow keys', async () => {
    const { wrapper } = await open({ routeKey: 'breakpoints', routeTab: 'breakpoints' });
    expect(tabs(wrapper).map((tab) => tab.text())).toEqual(['Breakpoints', 'Device presets', 'Breakpoint analyser']);
    (tabs(wrapper)[0]!.element as HTMLElement).focus();
    await tabs(wrapper)[0]!.trigger('keydown', { key: 'ArrowRight' });
    await settle();
    expect(document.activeElement).toBe(tabs(wrapper)[1]!.element);
  });

  it('opens a deep link to a key in its category and tab, highlighted and focused', async () => {
    const { wrapper } = await open({ routeKey: 'layoutModes.minimap' });
    expect(wrapper.get('[aria-current="page"]').attributes('data-category')).toBe('layout');
    expect(selectedTab(wrapper)).toBe('Further modes');
    expect(wrapper.get('[data-key="layoutModes.minimap"]').attributes('data-highlighted')).toBe('true');
    expect(document.activeElement?.closest('[data-key]')?.getAttribute('data-key')).toBe('layoutModes.minimap');
    await wrapper.setProps({ routeKey: 'layout', routeTab: 'zoom' });
    await settle();
    expect(selectedTab(wrapper)).toBe('Zoom');
    // Without a tab, `layout` is the key of the grid.
    await wrapper.setProps({ routeKey: 'layout', routeTab: null });
    await settle();
    expect(selectedTab(wrapper)).toBe('Grid');
  });

  it('searches every tab and a hit opens its tab', async () => {
    const { wrapper } = await open({ routeKey: 'layout', routeTab: 'presets' });
    await wrapper.get('[data-testid="search"]').setValue('minimap');
    await settle();
    expect(wrapper.find('[data-testid="settings-tabs"]').exists()).toBe(false);
    const keys = wrapper.findAll('[data-key]').map((field) => field.attributes('data-key'));
    expect(keys).toContain('layoutModes.minimap');
    const card = wrapper.get('[data-section="layout.modes"]');
    expect(card.get('[data-testid="show-in"]').text()).toBe('Show in Layout › Further modes');
    await card.get('[data-testid="show-in"]').trigger('click');
    await settle();
    expect((wrapper.get('[data-testid="search"]').element as HTMLInputElement).value).toBe('');
    expect(selectedTab(wrapper)).toBe('Further modes');
    expect(wrapper.get('[data-key="layoutModes.minimap"]').attributes('data-highlighted')).toBe('true');
  });

  it('draws the grid beside its fields with the breakpoint names', async () => {
    const core = createFakeCore();
    core.values.set('breakpoints', {
      key: 'breakpoints',
      value: [{ id: 'mobile', label: 'Phone' }, { id: 'desktop' }],
      origin: 'project',
    });
    core.values.set('layout', {
      key: 'layout',
      value: {
        columns: 2,
        rows: 1,
        gap: 8,
        cells: [
          { breakpoint: 'mobile', col: 1, row: 1 },
          { breakpoint: 'desktop', col: 2, row: 1 },
        ],
      },
      origin: 'project',
    });
    const { wrapper } = await open({ routeKey: 'layout' }, core);
    const preview = wrapper.get('[data-testid="grid-preview"]');
    expect(preview.findAll('[data-testid="grid-preview-cell"]').map((cell) => cell.text())).toEqual([
      'Phone',
      'desktop',
    ]);
    expect(preview.get('[role="img"]').attributes('aria-label')).toBe('Grid preview: 2 columns, 1 row, 2 cells');
  });
});

describe('every category of more than one section in tabs (RB-080-71)', () => {
  afterEach(() => localStorage.clear());

  async function choose(wrapper: VueWrapper, category: string): Promise<void> {
    await wrapper.get(`[data-category="${category}"]`).trigger('click');
    await settle();
  }

  it('shows Engines one section at a time', async () => {
    const { wrapper } = await open();
    await choose(wrapper, 'engines');
    expect(tabs(wrapper).map((tab) => tab.text())).toEqual(['Chromium/CEF', 'Firefox', 'WebKit/Playwright', 'Node runtime', 'External windows']);
    expect(sections(wrapper)).toEqual(['engines.chromium']);
  });

  it('reopens a category at the tab it was left on', async () => {
    const { wrapper } = await open();
    await choose(wrapper, 'engines');
    await tabs(wrapper)[1]!.trigger('mousedown', { button: 0 });
    await settle();
    expect(sections(wrapper)).toEqual(['engines.firefox']);
    await choose(wrapper, 'sync');
    await choose(wrapper, 'engines');
    expect(selectedTab(wrapper)).toBe('Firefox');
    expect(sections(wrapper)).toEqual(['engines.firefox']);
  });

  it('opens a link to a key at its tab, whatever tab was left open', async () => {
    const { wrapper } = await open();
    await choose(wrapper, 'engines');
    await tabs(wrapper)[1]!.trigger('mousedown', { button: 0 });
    await settle();
    await wrapper.setProps({ routeKey: 'engines.external.scaleToCell' });
    await settle();
    expect(selectedTab(wrapper)).toBe('External windows');
    expect(wrapper.get('[data-key="engines.external.scaleToCell"]').attributes('data-highlighted')).toBe('true');
  });

  it('puts every section on one page when settings.sectionTabs is off', async () => {
    const core = createFakeCore();
    core.values.set('settings.sectionTabs', { key: 'settings.sectionTabs', value: false, origin: 'user' });
    const { wrapper } = await open({}, core);
    await choose(wrapper, 'engines');
    expect(wrapper.find('[data-testid="settings-tabs"]').exists()).toBe(false);
    expect(sections(wrapper)).toEqual([
      'engines.chromium',
      'engines.firefox',
      'engines.webkit',
      'engines.node',
      'engines.external',
    ]);
  });

  it('counts only-changed hits across every tab, tabs hidden', async () => {
    const core = createFakeCore({ origins: { 'engines.external.scaleToCell': 'user' } });
    const { wrapper } = await open({}, core);
    await choose(wrapper, 'engines');
    await wrapper.get('[data-testid="only-changed"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="settings-tabs"]').exists()).toBe(false);
    expect(wrapper.findAll('[data-key]').map((field) => field.attributes('data-key'))).toContain(
      'engines.external.scaleToCell',
    );
  });
});

describe('the breakpoints as one-line entries, written to the project (RB-080-13)', () => {
  function withBreakpoints(): FakeCore {
    const core = createFakeCore();
    core.values.set('breakpoints', {
      key: 'breakpoints',
      value: [
        { id: 'mobile', width: 375, height: 667, dpr: 2, touch: true },
        { id: 'desktop', width: 1440, height: 900 },
      ],
      origin: 'project',
    });
    return core;
  }

  function written(core: FakeCore): unknown[] {
    return core.sent.flatMap((m) => (m.type === 'setSetting' && m.key === 'breakpoints' ? [m] : []));
  }

  function ids(message: unknown): string[] {
    return ((message as { value: { id: string }[] }).value ?? []).map((item) => item.id);
  }

  it('shows each breakpoint as its summary', async () => {
    const { wrapper } = await open({ routeKey: 'breakpoints' }, withBreakpoints());
    const lines = wrapper.findAll('[data-key="breakpoints"] [data-testid="list-toggle"]').map((line) => line.text());
    expect(lines).toEqual(['mobile · 375 × 667 · DPR 2 · Touch', 'desktop · 1440 × 900']);
  });

  it('adds, duplicates, moves, edits and deletes, each saved in config.json', async () => {
    const core = withBreakpoints();
    const { wrapper } = await open({ routeKey: 'breakpoints' }, core);
    const field = () => wrapper.get('[data-key="breakpoints"]');

    await field().get('[data-testid="list-add"]').trigger('click');
    await settle();
    expect(written(core)[0]).toMatchObject({ scope: 'project' });
    expect(ids(written(core)[0])).toEqual(['mobile', 'desktop', '']);

    await field().findAll('[data-testid="list-toggle"]')[0]!.trigger('click');
    await field().get('[data-testid="list-duplicate"]').trigger('click');
    await settle();
    expect(ids(written(core)[1])).toEqual(['mobile', 'mobile-copy', 'desktop', '']);

    await field().findAll('[data-testid="list-down"]')[2]!.trigger('click');
    await settle();
    expect(ids(written(core)[2])).toEqual(['mobile', 'mobile-copy', '', 'desktop']);

    const width = field().get('input#setting-breakpoints-0-width');
    await width.setValue('390');
    await settle();
    expect((written(core)[3] as { value: { width: number }[] }).value[0]!.width).toBe(390);

    await field().get('[data-testid="list-remove"]').trigger('click');
    await field().get('[data-testid="list-remove-confirm"]').trigger('click');
    await settle();
    expect(ids(written(core)[4])).toEqual(['mobile-copy', '', 'desktop']);
    expect(core.values.get('breakpoints')?.origin).toBe('project');
  });
});

describe('Save diagnostics in Settings > Advanced > Diagnostics (RB-080-73)', () => {
  it('stands above the crash dump setting', async () => {
    const { wrapper } = await open({ routeKey: 'logs.crashDumpsInBundle' });
    const card = wrapper.get('[data-section="advanced.diagnostics"]');
    expect(card.get('[data-testid="diagnostics-save"]').text()).toBe('Save diagnostics…');
    const html = card.html();
    expect(html.indexOf('diagnostics-save')).toBeLessThan(html.indexOf('data-key="logs.crashDumpsInBundle"'));
  });
});

// Review 2: the values of a pick list follow the core while the window is open.
describe('the values of the x-options sources (RB-080-72)', () => {
  function asked(core: FakeCore): number {
    return core.sent.filter((message) => message.type === 'getSettingSources').length;
  }

  it('are read with the schema, again on settingsChanged and on settingSourcesChanged', async () => {
    const core = createFakeCore();
    core.values.set('plugins.disabled', { key: 'plugins.disabled', value: ['seo'], origin: 'user' });
    core.sources.plugins = { options: [{ value: 'console', label: 'Console' }] };
    const { wrapper } = await open({ routeKey: 'plugins.disabled' }, core);
    const field = () => wrapper.get('[data-key="plugins.disabled"]');
    expect(asked(core)).toBe(1);
    expect(field().find('[data-testid="pick-missing"]').exists()).toBe(true);

    core.sources.plugins = { options: [{ value: 'seo', label: 'SEO' }] };
    core.emit({ type: 'settingsChanged', keys: ['plugins.disabled'] });
    await settle();
    expect(asked(core)).toBe(2);
    expect(field().get('[data-testid="pick-name"]').text()).toBe('SEO');
    expect(field().find('[data-testid="pick-missing"]').exists()).toBe(false);

    core.emit({ type: 'settingSourcesChanged' });
    await settle();
    expect(asked(core)).toBe(3);
  });
});
