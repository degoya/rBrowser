import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { JsonValue } from '../ipc/protocol';
import SettingsPage from './SettingsPage.vue';
import { createFakeCore, realDefs } from './test/fakeCore';
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

describe('the settings page', () => {
  it('lists every category and renders every key of the schema once', async () => {
    const { wrapper } = await open();
    const categories = wrapper.findAll('[data-category]').map((b) => b.attributes('data-category'));
    expect(categories).toHaveLength(18);
    const rendered: string[] = [];
    for (const id of categories) {
      await wrapper.get(`[data-category="${id}"]`).trigger('click');
      await settle();
      // Layout and Breakpoints & Devices show one section per sub-tab (RB-080-13).
      const tabs = wrapper.findAll('[data-testid="settings-tabs"] button[role="tab"]');
      for (const tab of tabs.length > 0 ? tabs : [null]) {
        await tab?.trigger('mousedown', { button: 0 });
        await settle();
        rendered.push(...wrapper.findAll('[data-key]').map((field) => field.attributes('data-key') ?? ''));
      }
    }
    const keys = realDefs().map((def) => def.key);
    expect([...rendered].sort()).toEqual([...keys].sort());
  });

  // RB-080-88: the categories are a vertical Nuxt UI navigation, the search a `UInput`.
  it('lists the categories in a named vertical navigation and opens one on click', async () => {
    const { wrapper } = await open();
    const nav = wrapper.get('[data-testid="settings-nav"]');
    expect(nav.attributes('aria-label')).toBe('Settings categories');
    expect(nav.attributes('data-orientation')).toBe('vertical');
    expect(wrapper.get('[aria-current="page"]').attributes('data-category')).toBe('general');
    await wrapper.get('[data-category="sync"]').trigger('click');
    await settle();
    expect(wrapper.get('[aria-current="page"]').attributes('data-category')).toBe('sync');
    const search = wrapper.get('[data-testid="search"]');
    expect(search.element.tagName).toBe('INPUT');
    expect(search.attributes('type')).toBe('search');
  });

  it('warns under Security & Privacy while the Chromium sandbox is off and links to it', async () => {
    const core = createFakeCore();
    core.values.set('engines.chromium.sandbox', { key: 'engines.chromium.sandbox', value: false, origin: 'user' });
    const { wrapper } = await open({ routeKey: 'security' }, core);
    const warning = wrapper.get('[data-testid="sandbox-warning"]');
    expect(warning.attributes('role')).toBe('alert');
    expect(warning.text()).toContain('sandbox is off');
    await warning.get('button').trigger('click');
    await settle();
    expect(wrapper.get('[data-key="engines.chromium.sandbox"]').attributes('data-highlighted')).toBe('true');
  });

  it('shows no sandbox warning while the sandbox is on', async () => {
    const { wrapper } = await open({ routeKey: 'security' });
    expect(wrapper.find('[data-testid="sandbox-warning"]').exists()).toBe(false);
  });

  it('finds sync.scroll when searching for scroll', async () => {
    const { wrapper } = await open();
    await wrapper.get('[data-testid="search"]').setValue('scroll');
    await settle();
    const keys = wrapper.findAll('[data-key]').map((field) => field.attributes('data-key'));
    expect(keys).toContain('sync.scroll');
    expect(keys).not.toContain('ui.theme');
  });

  it('says the number of hits in a status region that is there before the search (RB-090-22)', async () => {
    const { wrapper } = await open();
    const status = wrapper.get('[data-testid="search-status"]');
    expect(status.attributes('role')).toBe('status');
    expect(status.text()).toBe('');
    await wrapper.get('[data-testid="search"]').setValue('scroll');
    await settle();
    const hits = wrapper.findAll('[data-key]').length;
    expect(wrapper.get('[data-testid="search-status"]').element).toBe(status.element);
    expect(status.text()).toBe(`Found: ${hits}`);
    await wrapper.get('[data-testid="search"]').setValue('');
    await settle();
    expect(status.text()).toBe('');
  });

  it('badges a value of config.local.json as Local and resets it there', async () => {
    const core = createFakeCore({ origins: { 'sync.scroll': 'local' } });
    const { wrapper } = await open({ routeKey: 'sync.scroll' }, core);
    const field = wrapper.get('[data-key="sync.scroll"]');
    expect(field.get('[data-testid="origin"]').attributes('data-origin')).toBe('local');
    expect(field.get('[data-testid="origin"]').text()).toBe('Local');
    await field.get('[data-testid="reset"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'setSetting', key: 'sync.scroll', value: null, scope: 'local' });
    expect(wrapper.get('[data-key="sync.scroll"] [data-testid="origin"]').attributes('data-origin')).toBe(
      'default',
    );
  });

  it('locks ignoreCertificateErrors out of the project and says why', async () => {
    const { wrapper } = await open({ routeKey: 'network.ignoreCertificateErrors' });
    const field = wrapper.get('[data-key="network.ignoreCertificateErrors"]');
    // RB-080-88: a Nuxt UI `URadioGroup`; the locked scope is a disabled radio.
    const project = field.get('[role="radio"][value="project"]');
    expect(project.attributes('disabled')).toBeDefined();
    expect(field.get('[role="radio"][value="local"]').attributes('aria-checked')).toBe('true');
    expect(field.get('[data-testid="scope-reason"]').text()).toContain('Only on this machine');
  });

  it('opens a deep link at its section with the field highlighted', async () => {
    const { wrapper } = await open({ routeKey: 'sync.scroll' });
    expect(wrapper.get('[aria-current="page"]').attributes('data-category')).toBe('sync');
    const field = wrapper.get('[data-key="sync.scroll"]');
    expect(field.attributes('data-highlighted')).toBe('true');
    expect(document.activeElement?.closest('[data-key]')?.getAttribute('data-key')).toBe('sync.scroll');
    await wrapper.setProps({ routeKey: 'layout' });
    await settle();
    expect(wrapper.get('[aria-current="page"]').attributes('data-category')).toBe('layout');
  });

  it('opens an area link at its section, on its tab, with the section highlighted (RB-090-85)', async () => {
    const { wrapper } = await open({ routeKey: 'capture', routeSection: 'sitemap' });
    expect(wrapper.get('[aria-current="page"]').attributes('data-category')).toBe('capture');
    const card = wrapper.get('[data-section="capture.sitemap"]');
    expect(card.attributes('data-highlighted')).toBe('true');
    expect(wrapper.find('[data-section="capture.screenshots"]').exists()).toBe(false);
    // A key link afterwards highlights the field, not the section.
    await wrapper.setProps({ routeKey: 'sitemap.maxPages', routeSection: null });
    await settle();
    expect(wrapper.get('[data-section="capture.sitemap"]').attributes('data-highlighted')).toBeUndefined();
    expect(wrapper.get('[data-key="sitemap.maxPages"]').attributes('data-highlighted')).toBe('true');
  });

  it('opens an area link at its section with the section tabs off too', async () => {
    const core = createFakeCore();
    core.values.set('settings.sectionTabs', { key: 'settings.sectionTabs', value: false, origin: 'user' });
    const { wrapper } = await open({ routeKey: 'capture', routeSection: 'env-compare' }, core);
    expect(wrapper.find('[data-section="capture.screenshots"]').exists()).toBe(true);
    expect(wrapper.get('[data-section="capture.env-compare"]').attributes('data-highlighted')).toBe('true');
  });

  it('shows a refused value at the field and keeps the old one', async () => {
    const core = createFakeCore();
    const { wrapper } = await open({ routeKey: 'logs.maxFiles' }, core);
    const input = wrapper.get('[data-key="logs.maxFiles"] input#setting-logs-maxFiles');
    // A number commits on leaving the field (RB-080-88: `UInputNumber`).
    await input.setValue('-3');
    await input.trigger('blur');
    await settle();
    expect(core.sent.filter((m) => m.type === 'setSetting')).toEqual([
      { type: 'setSetting', key: 'logs.maxFiles', value: -3, scope: 'user' },
    ]);
    const issues = wrapper.get('[data-key="logs.maxFiles"] [data-testid="issues"]');
    expect(issues.text()).toContain('below the minimum');
    expect(core.values.get('logs.maxFiles')?.origin).toBe('default');
  });

  it('writes a project value through its control', async () => {
    const core = createFakeCore();
    const { wrapper } = await open({ routeKey: 'layout' }, core);
    const gap = wrapper.get('[data-key="layout"] input#setting-layout-gap');
    await gap.setValue('16');
    await gap.trigger('blur');
    await settle();
    const write = core.sent.find((m) => m.type === 'setSetting');
    expect(write).toMatchObject({ type: 'setSetting', key: 'layout', scope: 'project' });
    expect((write as unknown as { value: { gap: number } }).value.gap).toBe(16);
  });

  it('shows the JSON of a scope', async () => {
    const core = createFakeCore({ origins: { 'sync.scroll': 'local' } });
    const { wrapper } = await open({}, core);
    await wrapper.get('[data-testid="json-toggle"]').trigger('click');
    await settle();
    await wrapper.get('[data-testid="json-panel"] [data-scope="local"]').trigger('click');
    await settle();
    expect(JSON.parse(wrapper.get('[data-testid="json-text"]').text())).toEqual({ sync: { scroll: true } });
  });

  it('reaches every control with Tab: each has a name', async () => {
    const { wrapper } = await open({ routeKey: 'sync' });
    const controls = wrapper.element.querySelectorAll('button, input, select, textarea, [role="switch"]');
    expect(controls.length).toBeGreaterThan(10);
    for (const control of controls) {
      // The tabs of a category and the radios of a scope rove: one is in the Tab order, the
      // arrow keys reach the rest.
      if (control.getAttribute('role') !== 'tab' && control.getAttribute('role') !== 'radio') {
        expect(control.getAttribute('tabindex')).not.toBe('-1');
      }
      const named =
        control.getAttribute('aria-label') ??
        (control.id ? document.querySelector(`label[for="${control.id}"]`)?.textContent : null) ??
        control.closest('label')?.textContent ??
        control.textContent;
      expect(named?.trim(), control.outerHTML).toBeTruthy();
    }
  });

  // RB-090-61: the trash icon of a breakpoint says which cells go along before the core removes them.
  it('names the layout cells a breakpoint takes along in its delete confirmation', async () => {
    const core = createFakeCore();
    const set = (key: string, value: JsonValue): void => {
      core.values.set(key, { key, value, origin: 'project' });
    };
    set('breakpoints', [
      { id: 'mobile', width: 393, height: 852 },
      { id: 'tablet', width: 768, height: 1024 },
    ]);
    set('layout', {
      columns: 2,
      rows: 1,
      cells: [
        { breakpoint: 'mobile', col: 1, row: 1 },
        { breakpoint: 'tablet', col: 2, row: 1 },
      ],
      hidden: ['tablet'],
    });
    const { wrapper } = await open({ routeKey: 'breakpoints' }, core);
    const field = wrapper.get('[data-key="breakpoints"]');
    await field.findAll('[data-testid="list-trash"]')[1]!.trigger('click');
    await settle();
    expect(field.get('[data-testid="list-confirm-note"]').text()).toBe(
      'Also removed from 1 layout cell and 1 list of hidden views.',
    );
    await field.findAll('[data-testid="list-trash"]')[0]!.trigger('click');
    await settle();
    expect(field.get('[data-testid="list-confirm-note"]').text()).toBe('Also removed from 1 layout cell.');
  });
});
