// Running checks again (RB-094-17): one check from its row in the progress details or from the
// plugin filter, a selection of checks from the details - each over the last run's target and
// page, so the core merges the result into the last run.
import USelect from '@nuxt/ui/components/Select.vue';
import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../test/mountShell';
import FindingsPanel from './FindingsPanel.vue';
import { fakeChannel, sampleMessages, sampleProgress, sampleRun } from './testing';
import type { FindingsRun } from './types';

/** The finished progress of the sample run: both plugins through in both views. */
function finished() {
  return sampleProgress({
    finished: true,
    jobs: [
      { plugin: 'seo', view: 1, breakpoint: 'mobile', state: 'done', findings: 1 },
      { plugin: 'seo', view: 2, breakpoint: 'tablet', state: 'done', findings: 0 },
      { plugin: 'a11y-axe', view: 1, breakpoint: 'mobile', state: 'done', findings: 1 },
      { plugin: 'a11y-axe', view: 2, breakpoint: 'tablet', state: 'done', findings: 0 },
    ],
  });
}

async function withRun(run: FindingsRun = sampleRun()) {
  const core = fakeChannel();
  const wrapper = mount(FindingsPanel, {
    props: { channel: core.channel },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  core.emit({ type: 'findings', run, messages: sampleMessages });
  core.emit(finished());
  await settle();
  return { core, wrapper };
}

async function openDetails(wrapper: VueWrapper): Promise<void> {
  await wrapper.get('[data-testid="progress-details"]').trigger('click');
  await settle();
}

describe('FindingsPanel running checks again', () => {
  it('runs one check again from its row in the progress details', async () => {
    const { core, wrapper } = await withRun();
    await openDetails(wrapper);
    wrapper.get('[data-testid="progress-seo"]').element.closest('button')?.click();
    await settle();
    const again = wrapper.get('[data-testid="rerun-seo"]');
    expect(again.attributes('aria-label')).toBe('Run seo again');
    await again.trigger('click');
    await settle();
    expect(core.sent).toEqual([{ type: 'runChecks', target: 'all', plugins: ['seo'] }]);
    wrapper.unmount();
  });

  it('runs the chosen checks again, and nothing while none is chosen', async () => {
    const { core, wrapper } = await withRun();
    await openDetails(wrapper);
    const selected = wrapper.get('[data-testid="rerun-selected"]');
    expect(selected.attributes('disabled')).toBeDefined();
    await wrapper.get('[data-testid="rerun-choice-a11y-axe"]').trigger('click');
    await wrapper.get('[data-testid="rerun-choice-seo"]').trigger('click');
    await settle();
    expect(selected.attributes('disabled')).toBeUndefined();
    await selected.trigger('click');
    await settle();
    expect(core.sent).toHaveLength(1);
    const sent = core.sent[0] as { type: string; plugins?: string[] };
    expect(sent.type).toBe('runChecks');
    expect([...(sent.plugins ?? [])].sort()).toEqual(['a11y-axe', 'seo']);
    wrapper.unmount();
  });

  it('runs the check the plugin filter shows again', async () => {
    const { core, wrapper } = await withRun();
    expect(wrapper.find('[data-testid="rerun-filtered"]').exists()).toBe(false);
    wrapper.findAllComponents(USelect)[0]!.vm.$emit('update:modelValue', 'a11y-axe');
    await settle();
    const again = wrapper.get('[data-testid="rerun-filtered"]');
    expect(again.attributes('aria-label')).toBe('Run a11y-axe again');
    await again.trigger('click');
    await settle();
    expect(core.sent).toEqual([{ type: 'runChecks', target: 'all', plugins: ['a11y-axe'] }]);
    wrapper.unmount();
  });

  it('runs again over the last run target and page, so the core merges it', async () => {
    const { core, wrapper } = await withRun({ ...sampleRun(), target: 2, url: 'https://site.test/a' });
    wrapper.findAllComponents(USelect)[0]!.vm.$emit('update:modelValue', 'seo');
    await settle();
    await wrapper.get('[data-testid="rerun-filtered"]').trigger('click');
    await settle();
    expect(core.sent).toEqual([{ type: 'runChecks', target: 2, plugins: ['seo'], url: 'https://site.test/a' }]);
    wrapper.unmount();
  });

  it('offers nothing to run again while checks run', async () => {
    const { core, wrapper } = await withRun();
    wrapper.findAllComponents(USelect)[0]!.vm.$emit('update:modelValue', 'seo');
    await settle();
    await wrapper.get('[data-testid="rerun-filtered"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="rerun-filtered"]').attributes('disabled')).toBeDefined();
    core.emit(sampleProgress());
    await settle();
    expect(wrapper.find('[data-testid="rerun-selected"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid^="rerun-seo"]').exists()).toBe(false);
    wrapper.unmount();
  });
});
