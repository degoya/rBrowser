import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../test/mountShell';
import FindingsPanel from './FindingsPanel.vue';
import { fakeChannel, sampleMessages, sampleRun } from './testing';
import type { FindingsRun } from './types';

/** The sample run with a width sweep finding: 700-740 px, its width 720, in view 2. */
function sweepRun(): FindingsRun {
  const run = sampleRun();
  run.plugins.push('width-sweep');
  run.findings.push({
    id: 'w',
    ruleId: 'overflow',
    plugins: ['width-sweep'],
    severity: 'error',
    message: { key: 'plugin.seo.findings.title' },
    views: [2],
    breakpoints: ['tablet'],
    element: { selector: '.hero img' },
    data: { kind: 'overflow', from: 700, to: 740, width: 720 },
  });
  return run;
}

async function mounted() {
  const core = fakeChannel();
  const wrapper = mount(FindingsPanel, {
    props: { channel: core.channel },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  core.emit({ type: 'findings', run: sweepRun(), messages: sampleMessages });
  await settle();
  return { core, wrapper };
}

describe('FindingsPanel: Show at N px (RB-093-01)', () => {
  it('is offered only for a finding that names a width', async () => {
    const { wrapper } = await mounted();
    expect(wrapper.findAll('[data-testid^="show-width-"]').map((button) => button.attributes('data-testid'))).toEqual([
      'show-width-w',
    ]);
    const button = wrapper.get('[data-testid="show-width-w"]');
    expect(button.attributes('aria-label')).toBe('Show at 720 px');
    expect(button.attributes('title')).toContain('Escape in the view or Cancel in the activity bar');
    wrapper.unmount();
  });

  it("marks the element and runs the plugin's show command on the finding's view", async () => {
    const { core, wrapper } = await mounted();
    await wrapper.get('[data-testid="show-width-w"]').trigger('click');
    await settle();
    expect(core.sent).toEqual([
      { type: 'highlightFinding', findingId: 'w', selector: '.hero img', views: [2] },
      { type: 'runCommand', id: 'plugin.width_sweep.show', args: { target: 2, width: 720 } },
    ]);
    expect(wrapper.get('[data-testid="finding-w"]').attributes('aria-pressed')).toBe('true');
    wrapper.unmount();
  });
});
