// The Run button disables itself while checks run (RB-090-23): the focus goes to Cancel, the one
// action that ends the run, and back to Run when the run is over - never to the body.
import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { ref } from 'vue';
import { describe, expect, it, vi } from 'vitest';
import { createShellI18n } from '../i18n';
import { SHELL_KEY } from '../state/shell';
import { settle } from '../test/mountShell';
import FindingsPanel from './FindingsPanel.vue';
import { fakeChannel, sampleMessages, sampleProgress, sampleRun } from './testing';

function mountPanel() {
  const core = fakeChannel();
  const shell = { send: vi.fn(async () => true), settings: ref({}) };
  const wrapper = mount(FindingsPanel, {
    props: { channel: core.channel },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()], provide: { [SHELL_KEY]: shell } },
  });
  return { core, wrapper };
}

describe('the focus around a check run', () => {
  it('moves from Run to Cancel while the run goes, and back to Run after it', async () => {
    const { core, wrapper } = mountPanel();
    await settle();
    const run = wrapper.get('[data-testid="run"]');
    (run.element as HTMLElement).focus();
    await run.trigger('click');
    await settle();
    expect(run.attributes('disabled')).toBeDefined();
    core.emit(sampleProgress());
    await settle();
    expect(document.activeElement).toBe(wrapper.get('[data-testid="cancel"]').element);

    core.emit(sampleProgress({ finished: true }));
    core.emit({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    await settle();
    expect(wrapper.find('[data-testid="cancel"]').exists()).toBe(false);
    expect(document.activeElement).toBe(wrapper.get('[data-testid="run"]').element);
    wrapper.unmount();
  });

  it('leaves the focus alone when the reader moved it during the run', async () => {
    const { core, wrapper } = mountPanel();
    await settle();
    const run = wrapper.get('[data-testid="run"]');
    (run.element as HTMLElement).focus();
    await run.trigger('click');
    core.emit(sampleProgress());
    await settle();
    const rules = wrapper.get('[data-testid="rules"]').element as HTMLElement;
    rules.focus();
    core.emit(sampleProgress({ finished: true }));
    core.emit({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    await settle();
    expect(document.activeElement).toBe(rules);
    wrapper.unmount();
  });
});
