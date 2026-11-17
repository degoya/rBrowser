import USelect from '@nuxt/ui/components/Select.vue';
import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../test/mountShell';
import AnalyzerWizard from './AnalyzerWizard.vue';
import { fakeCore, proposal } from './testing';

function mountWizard(projectOpen: boolean | null = true, core = fakeCore()) {
  const wrapper = mount(AnalyzerWizard, {
    props: { client: core.client, projectOpen },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  return { core, wrapper };
}

describe('AnalyzerWizard', () => {
  it('asks for an analysis with the typed pages and mode', async () => {
    const { core, wrapper } = mountWizard();
    await wrapper.find('[data-testid="urls"]').setValue('http://site.test/about');
    const mode = wrapper.findComponent(USelect);
    expect(mode.exists()).toBe(true);
    expect(wrapper.find('select').exists()).toBe(false);
    mode.vm.$emit('update:modelValue', 'simplified');
    await wrapper.find('form').trigger('submit');
    await settle();
    expect(core.sent).toEqual([
      { type: 'analyzeBreakpoints', urls: ['http://site.test/about'], mode: 'simplified' },
    ]);
    wrapper.unmount();
  });

  it('shows the diff with a preview and writes only on the button', async () => {
    const { core, wrapper } = mountWizard();
    core.emit({ type: 'breakpointProposal', ...proposal(true) });
    await settle();
    expect(wrapper.find('[data-testid="diff-note"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="row-mobile"]').exists()).toBe(false);
    expect(wrapper.findAll('[data-testid="preview"] > div')).toHaveLength(2);
    expect(core.sent).toEqual([]);

    // Nuxt UI checkboxes (RB-080-88): the new views ticked, "replace" not.
    const md = wrapper.get('[data-testid="row-md"]');
    expect(md.attributes('role')).toBe('checkbox');
    expect(md.attributes('aria-checked')).toBe('true');
    const replace = wrapper.get('[data-testid="replace"]');
    expect(replace.attributes('aria-checked')).toBe('false');
    await md.trigger('click');
    await replace.trigger('click');
    await settle();
    expect(md.attributes('aria-checked')).toBe('false');
    expect(replace.attributes('aria-checked')).toBe('true');
    await wrapper.find('[data-testid="apply"]').trigger('click');
    await settle();
    expect(core.sent).toEqual([
      { type: 'applyBreakpoints', proposalId: 7, selected: ['desktop'], replace: true },
    ]);
    wrapper.unmount();
  });

  it('names the project file of a declared boundary and lists the hints', async () => {
    const { core, wrapper } = mountWizard();
    const declared = proposal(false);
    declared.detected = [
      { width: 1920, kind: 'min', label: '3xl', count: 0, source: 'tailwind.config.js: 3xl', heuristic: false },
    ];
    declared.hints = [
      {
        path: 'tailwind.config.js',
        key: 'plugin.breakpoint-analyzer.hints.dynamic',
        params: { name: 'lg', path: 'tailwind.config.js' },
      },
    ];
    core.emit({ type: 'breakpointProposal', ...declared });
    await settle();
    // The stylesheets and boundaries fold away in a collapsible (RB-080-88), closed at first.
    const toggle = wrapper.get('[data-testid="sources-toggle"]');
    expect(toggle.attributes('aria-expanded')).toBe('false');
    expect(wrapper.find('[data-testid="declared-1920"]').exists()).toBe(false);
    await toggle.trigger('click');
    await settle();
    expect(toggle.attributes('aria-expanded')).toBe('true');
    expect(wrapper.find('[data-testid="declared-1920"]').text()).toBe('(declared in tailwind.config.js: 3xl)');
    expect(wrapper.find('[data-testid="hints"]').text()).toContain('tailwind.config.js: lg is not statically readable');
    wrapper.unmount();
  });

  it('says so when the config has every breakpoint already', async () => {
    const { core, wrapper } = mountWizard();
    const known = proposal(true);
    known.breakpoints = known.breakpoints.map((view) => ({ ...view, isNew: false }));
    core.emit({ type: 'breakpointProposal', ...known });
    await settle();
    expect(wrapper.find('[data-testid="diff-note"]').text()).toContain('No new breakpoints');
    expect(wrapper.find('[data-testid="apply"]').attributes('disabled')).toBeDefined();
    wrapper.unmount();
  });

  it('draws the preview at one scale with the label and size under each view', async () => {
    const { core, wrapper } = mountWizard();
    core.emit({ type: 'breakpointProposal', ...proposal(false) });
    await settle();
    await wrapper.get('[data-testid="row-mobile"]').trigger('click');
    await settle();
    const cells = wrapper.findAll('[data-testid="preview-cell"]');
    expect(cells.map((cell) => cell.find('[data-testid="preview-label"]').text())).toEqual(['mobile', 'md', 'desktop']);
    expect(cells[0]!.text()).toContain('375 x 667');
    const widths = cells.map((cell) =>
      Number.parseFloat((cell.find('[data-testid="preview-box"]').element as HTMLElement).style.width),
    );
    expect(widths[0]).toBeLessThan(widths[1]!);
    expect(widths[1]).toBeLessThan(widths[2]!);
    wrapper.unmount();
  });

  it('analyses without a project but does not write, says why and offers the recent projects', async () => {
    const core = fakeCore({
      getSettings: {
        type: 'settings',
        values: [{ key: 'recentProjects', value: ['C:\\work\\shop', 'C:\\work\\blog'], origin: 'user' }],
      },
    });
    const { wrapper } = mountWizard(false, core);
    await settle();
    expect(wrapper.find('[data-testid="no-project"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="run"]').attributes('disabled')).toBeUndefined();
    core.emit({ type: 'breakpointProposal', ...proposal(false) });
    await settle();
    const apply = wrapper.find('[data-testid="apply"]');
    expect(apply.attributes('disabled')).toBeDefined();
    const reason = wrapper.find('[data-testid="apply-reason"]');
    expect(reason.text()).toContain('project');
    expect(apply.attributes('aria-describedby')).toBe(reason.attributes('id'));
    await apply.trigger('click');
    expect(core.sent.some((message) => message.type === 'applyBreakpoints')).toBe(false);

    await wrapper.find('[data-testid="open-recent"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({
      type: 'runCommand',
      id: 'project.open_recent',
      args: { path: 'C:\\work\\shop' },
    });
    wrapper.unmount();
  });

  it('draws no raw control: checkboxes, select and collapsible are Nuxt UI (RB-080-88)', async () => {
    const { core, wrapper } = mountWizard();
    core.emit({ type: 'breakpointProposal', ...proposal(true) });
    await settle();
    expect(wrapper.findAll('[role="checkbox"]').length).toBeGreaterThan(1);
    // reka's hidden form mirrors (`aria-hidden`) are no controls of their own.
    for (const raw of ['select:not([aria-hidden="true"])', 'details', 'input:not([aria-hidden="true"])']) {
      expect(wrapper.findAll(raw)).toHaveLength(0);
    }
    wrapper.unmount();
  });

  it('shows no project notice while the state is not known yet', async () => {
    const { core, wrapper } = mountWizard(null);
    await settle();
    expect(wrapper.find('[data-testid="no-project"]').exists()).toBe(false);
    expect(core.sent).toEqual([]);
    wrapper.unmount();
  });
});
