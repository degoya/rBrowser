import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../test/mountShell';
import AnalyzerWizard from './AnalyzerWizard.vue';
import { fakeCore, proposal } from './testing';

function mountWizard() {
  const core = fakeCore();
  const wrapper = mount(AnalyzerWizard, {
    props: { client: core.client },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  return { core, wrapper };
}

describe('AnalyzerWizard', () => {
  it('asks for an analysis with the typed pages and mode', async () => {
    const { core, wrapper } = mountWizard();
    await wrapper.find('[data-testid="urls"]').setValue('http://site.test/about');
    await wrapper.find('[data-testid="mode"]').setValue('simplified');
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

    await wrapper.find('[data-testid="row-md"] input').setValue(false);
    await wrapper.find('[data-testid="replace"]').setValue(true);
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
});
