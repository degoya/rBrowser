import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../test/mountShell';
import FindingsPanel from './FindingsPanel.vue';
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
    await wrapper.find('[data-testid="severity-warning"]').setValue(false);
    await wrapper.find('[data-testid="severity-info"]').setValue(false);
    expect(wrapper.findAll('[data-finding]').map((b) => b.attributes('data-testid'))).toEqual(['finding-a']);
    await wrapper.find('[data-testid="severity-warning"]').setValue(true);
    await wrapper.find('[data-testid="plugin"]').setValue('a11y-axe');
    expect(wrapper.findAll('[data-finding]')).toHaveLength(1);
    await wrapper.find('[data-testid="plugin"]').setValue('');
    await wrapper.find('[data-testid="view"]').setValue('2');
    expect(wrapper.findAll('[data-finding]').map((b) => b.attributes('data-testid'))).toEqual(['finding-a']);
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
