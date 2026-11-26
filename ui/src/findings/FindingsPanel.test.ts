import USelect from '@nuxt/ui/components/Select.vue';
import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../test/mountShell';
import FindingsFilters from './FindingsFilters.vue';
import FindingsPanel from './FindingsPanel.vue';
import VisualStart from './VisualStart.vue';
import ScopePicker from '../scope/ScopePicker.vue';
import { fakeChannel, sampleMessages, sampleRun } from './testing';

function mountPanel() {
  const core = fakeChannel();
  const wrapper = mount(FindingsPanel, {
    props: { channel: core.channel },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  return { core, wrapper };
}

/** The filter selects in their order: plugin, then view. */
function filterSelect(wrapper: VueWrapper, which: 'plugin' | 'view') {
  // The filters' own selects: the scope pick lists above them are others (RB-099-10).
  const selects = wrapper.getComponent(FindingsFilters).findAllComponents(USelect);
  expect(selects).toHaveLength(2);
  return selects[which === 'plugin' ? 0 : 1]!;
}

/** The value a filter select shows. */
function selected(wrapper: VueWrapper, which: 'plugin' | 'view'): unknown {
  return (filterSelect(wrapper, which).props() as { modelValue: unknown }).modelValue;
}

async function choose(wrapper: VueWrapper, which: 'plugin' | 'view', value: unknown): Promise<void> {
  filterSelect(wrapper, which).vm.$emit('update:modelValue', value);
  await settle();
}

async function withRun() {
  const mounted = mountPanel();
  mounted.core.emit({ type: 'findings', run: sampleRun(), messages: sampleMessages });
  await settle();
  return mounted;
}

describe('FindingsPanel', () => {
  it('asks for a run of every check', async () => {
    const { core, wrapper } = mountPanel();
    expect(wrapper.find('[data-testid="no-run"]').exists()).toBe(true);
    await wrapper.find('[data-testid="run"]').trigger('click');
    await settle();
    expect(core.sent).toEqual([{ type: 'runChecks', target: 'all' }]);
    wrapper.unmount();
  });

  it('names what the project turned off, with the count and the list', async () => {
    const { core, wrapper } = mountPanel();
    core.emit({
      type: 'findings',
      run: sampleRun(),
      messages: sampleMessages,
      projectOff: {
        rules: ['seo.title'],
        ignore: [{ rule: 'a11y.*', url: 'http://localhost:5173/legacy/*' }],
      },
    });
    await settle();
    const note = wrapper.get('[data-testid="project-off"]');
    expect(note.text()).toContain('2 turned off by the project');
    // A collapsible (RB-080-88): the list shows once it is opened.
    const toggle = wrapper.get('[data-testid="project-off-toggle"]');
    expect(toggle.attributes('aria-expanded')).toBe('false');
    await toggle.trigger('click');
    await settle();
    expect(toggle.attributes('aria-expanded')).toBe('true');
    expect(note.text()).toContain('seo.title');
    expect(note.text()).toContain('a11y.*');
    expect(note.text()).toContain('http://localhost:5173/legacy/*');
    wrapper.unmount();
  });

  it('says nothing about the project when it turned nothing off', async () => {
    const { wrapper } = await withRun();
    expect(wrapper.find('[data-testid="project-off"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('groups the findings by rule with the plugin texts', async () => {
    const { wrapper } = await withRun();
    expect(wrapper.findAll('[data-testid^="rule-"]')).toHaveLength(2);
    expect(wrapper.find('[data-testid="finding-a"]').text()).toContain('2 images without alt text');
    expect(wrapper.find('[data-testid="summary"]').exists()).toBe(true);
    wrapper.unmount();
  });

  it('says in which breakpoints a finding is when it is not in all', async () => {
    const { wrapper } = await withRun();
    expect(wrapper.find('[data-testid="finding-a"] [data-testid="only-in"]').exists()).toBe(false);
    const note = wrapper.find('[data-testid="finding-b"] [data-testid="only-in"]');
    expect(note.text()).toContain('mobile, desktop');
    expect(note.text()).not.toContain('tablet');
    wrapper.unmount();
  });

  it('filters by severity, plugin and view', async () => {
    const { wrapper } = await withRun();
    const warning = wrapper.get('[data-testid="severity-warning"]');
    expect(warning.attributes('role')).toBe('checkbox');
    expect(warning.attributes('aria-checked')).toBe('true');
    await warning.trigger('click');
    await wrapper.get('[data-testid="severity-info"]').trigger('click');
    await settle();
    expect(warning.attributes('aria-checked')).toBe('false');
    expect(wrapper.findAll('[data-finding]').map((b) => b.attributes('data-testid'))).toEqual(['finding-a']);
    await warning.trigger('click');
    await choose(wrapper, 'plugin', 'a11y-axe');
    expect(wrapper.findAll('[data-finding]')).toHaveLength(1);
    await choose(wrapper, 'plugin', '*');
    await choose(wrapper, 'view', 2);
    expect(wrapper.findAll('[data-finding]').map((b) => b.attributes('data-testid'))).toEqual(['finding-a']);
    wrapper.unmount();
  });

  it('falls back to all plugins and views when the next run no longer has the choice (RB-094-14)', async () => {
    const { core, wrapper } = await withRun();
    await choose(wrapper, 'plugin', 'a11y-axe');
    await choose(wrapper, 'view', 3);
    const next = sampleRun();
    next.plugins = ['seo'];
    next.findings = next.findings.filter((finding) => !finding.plugins.includes('a11y-axe'));
    next.views = next.views.filter((checked) => checked.id !== 3);
    core.emit({ type: 'findings', run: next, messages: sampleMessages });
    await settle();
    expect(selected(wrapper, 'plugin')).toBe('*');
    expect(selected(wrapper, 'view')).toBe('*');
    expect(wrapper.findAll('[data-finding]')).toHaveLength(next.findings.length);
    wrapper.unmount();
  });

  it('keeps a filter the next run still offers', async () => {
    const { core, wrapper } = await withRun();
    await choose(wrapper, 'plugin', 'seo');
    core.emit({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    await settle();
    expect(selected(wrapper, 'plugin')).toBe('seo');
    wrapper.unmount();
  });

  it('offers every plugin and view, each select with all of them first (RB-080-88)', async () => {
    const { wrapper } = await withRun();
    const labels = (which: 'plugin' | 'view') =>
      (filterSelect(wrapper, which).props() as { items: { label: string }[] }).items.map((item) => item.label);
    expect(labels('plugin')).toEqual(['All plugins', 'a11y-axe', 'seo']);
    expect(labels('view')).toEqual(['All views', 'mobile', 'tablet', 'desktop']);
    expect((filterSelect(wrapper, 'plugin').props() as { modelValue: unknown }).modelValue).toBe('*');
    expect(wrapper.get('[data-testid="plugin"]').text()).toContain('All plugins');
    wrapper.unmount();
  });

  it('draws no raw control: Nuxt UI checkboxes, selects and collapsibles (RB-080-88)', async () => {
    const { core, wrapper } = mountPanel();
    core.emit({
      type: 'findings',
      run: sampleRun(),
      messages: sampleMessages,
      projectOff: { rules: ['seo.title'], ignore: [] },
    });
    await settle();
    // reka's hidden form mirrors (`aria-hidden`) are no controls of their own.
    for (const raw of ['select:not([aria-hidden="true"])', 'details', 'input:not([aria-hidden="true"])']) {
      expect(wrapper.findAll(raw)).toHaveLength(0);
    }
    wrapper.unmount();
  });

  it('folds a rule away and keeps it folded while the filters change', async () => {
    const { wrapper } = await withRun();
    const rule = wrapper.get('[data-testid="rule-seo.images"]');
    const trigger = rule.element.closest('button');
    expect(trigger?.getAttribute('aria-expanded')).toBe('true');
    expect(wrapper.find('[data-testid="finding-a"]').exists()).toBe(true);
    trigger?.click();
    await settle();
    expect(wrapper.find('[data-testid="finding-a"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="finding-c"]').exists()).toBe(true);
    await wrapper.get('[data-testid="severity-info"]').trigger('click');
    await wrapper.get('[data-testid="severity-info"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="finding-a"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="finding-c"]').exists()).toBe(true);
    wrapper.unmount();
  });

  it('asks the core to mark the element in the views of the finding', async () => {
    const { core, wrapper } = await withRun();
    await wrapper.find('[data-testid="finding-b"]').trigger('click');
    await settle();
    expect(core.sent).toEqual([
      { type: 'highlightFinding', findingId: 'b', selector: 'img.logo', views: [1, 3] },
    ]);
    expect(wrapper.find('[data-testid="finding-b"]').attributes('aria-pressed')).toBe('true');
    wrapper.unmount();
  });

  it('opens the rule settings', async () => {
    const { core, wrapper } = await withRun();
    await wrapper.find('[data-testid="rules"]').trigger('click');
    await settle();
    expect(core.sent).toEqual([{ type: 'openSettings', key: 'checks.rules' }]);
    wrapper.unmount();
  });

  it('moves through the findings with the arrow keys', async () => {
    const { wrapper } = await withRun();
    const first = wrapper.find<HTMLButtonElement>('[data-testid="finding-a"]');
    first.element.focus();
    await first.trigger('keydown', { key: 'ArrowDown' });
    await settle();
    expect(document.activeElement?.getAttribute('data-testid')).toBe('finding-b');
    await wrapper.find('[data-testid="finding-b"]').trigger('keydown', { key: 'ArrowUp' });
    await settle();
    expect(document.activeElement?.getAttribute('data-testid')).toBe('finding-a');
    wrapper.unmount();
  });
});

describe('FindingsPanel scope (RB-099-10)', () => {
  const BREAKPOINTS = [
    { id: 'mobile', label: 'Mobile', width: 393, height: 852 },
    { id: 'desktop', label: 'Desktop', width: 1440, height: 900 },
  ];

  function mountScoped() {
    const core = fakeChannel();
    const wrapper = mount(FindingsPanel, {
      props: { channel: core.channel, breakpoints: BREAKPOINTS },
      attachTo: document.body,
      global: { plugins: [ui, createShellI18n()] },
    });
    return { core, wrapper };
  }

  afterEach(() => {
    window.localStorage.removeItem('rb.scope.checks');
    window.localStorage.removeItem('rb.scope.visual');
  });

  it('runs the checks over the scope chosen and names the views checked', async () => {
    const { core, wrapper } = mountScoped();
    const [checks] = wrapper.findAllComponents(ScopePicker);
    checks!.vm.$emit('update:modelValue', { mode: 'chosen', breakpoints: ['mobile'] });
    await settle();
    await wrapper.find('[data-testid="run"]').trigger('click');
    await settle();
    expect(core.sent).toEqual([{ type: 'runChecks', target: 'all', scope: ['mobile'] }]);
    core.emit({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    await settle();
    expect(wrapper.find('[data-testid="checked"]').exists()).toBe(true);
    checks!.vm.$emit('update:modelValue', { mode: 'chosen', breakpoints: [] });
    await settle();
    expect(wrapper.find('[data-testid="run"]').attributes('disabled')).toBeDefined();
    wrapper.unmount();
  });

  // Review: a run from `checks.run_visible` (K 3) or MCP over two of three breakpoints - "Run
  // again" stays on those views, so the core merges it.
  it('runs a check again over the views of the run shown', async () => {
    const { core, wrapper } = mountScoped();
    const shown = { ...sampleRun(), views: [{ id: 1, breakpoint: 'mobile' }] };
    core.emit({ type: 'findings', run: shown, messages: sampleMessages });
    await settle();
    wrapper.getComponent(FindingsFilters).findAllComponents(USelect)[0]!.vm.$emit('update:modelValue', 'seo');
    await settle();
    await wrapper.get('[data-testid="rerun-filtered"]').trigger('click');
    await settle();
    expect(core.sent).toEqual([{ type: 'runChecks', target: 'all', plugins: ['seo'], scope: ['mobile'] }]);
    wrapper.unmount();
  });

  it('starts visual regression over its own scope', async () => {
    const { core, wrapper } = mountScoped();
    const visual = wrapper.getComponent(VisualStart);
    await visual.find('[data-testid="visual-compare"]').trigger('click');
    visual.getComponent(ScopePicker).vm.$emit('update:modelValue', { mode: 'visible', breakpoints: [] });
    await settle();
    await visual.find('[data-testid="visual-update"]').trigger('click');
    await settle();
    expect(core.sent).toEqual([
      { type: 'runCommand', id: 'visual.compare' },
      { type: 'runCommand', id: 'visual.baseline_update', args: { scope: 'visible' } },
    ]);
    wrapper.unmount();
  });
});
