// The findings panel while checks run (RB-080-89) and its Help links (RB-080-92): the progress
// per plugin (RB-080-98), the time, the cancel and the summary line after it, the findings of a plugin as soon as it is through,
// the plural of the summary and the failure with its reason; Help goes to `app.open_link`.
import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { ref } from 'vue';
import { describe, expect, it, vi } from 'vitest';
import { createShellI18n } from '../i18n';
import type { ShellToCore } from '../ipc/protocol';
import { SHELL_KEY } from '../state/shell';
import { settle } from '../test/mountShell';
import FindingsPanel from './FindingsPanel.vue';
import { fakeChannel, sampleMessages, sampleProgress, sampleRun } from './testing';

function mountPanel(settings: Record<string, unknown> = {}) {
  const core = fakeChannel();
  const shellSent: ShellToCore[] = [];
  const shell = {
    send: vi.fn(async (message: ShellToCore) => {
      shellSent.push(message);
      return true;
    }),
    settings: ref(settings),
  };
  const wrapper = mount(FindingsPanel, {
    props: { channel: core.channel },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()], provide: { [SHELL_KEY]: shell } },
  });
  return { core, wrapper, shellSent };
}

describe('FindingsPanel while checks run', () => {
  it('says nothing about findings before the first run', async () => {
    const { wrapper } = mountPanel();
    await settle();
    expect(wrapper.find('[data-testid="summary"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="no-run"]').exists()).toBe(true);
    wrapper.unmount();
  });

  it('counts findings and plugins in the right plural', async () => {
    const { core, wrapper } = mountPanel();
    const one = sampleRun();
    one.plugins = ['seo'];
    one.summary = { ...one.summary, total: 1 };
    core.emit({ type: 'findings', run: one, messages: sampleMessages });
    await settle();
    expect(wrapper.get('[data-testid="summary"]').text()).toBe('1 finding from 1 plugin');
    core.emit({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    await settle();
    expect(wrapper.get('[data-testid="summary"]').text()).toBe('3 findings from 2 plugins');
    wrapper.unmount();
  });

  it('shows which check runs in which view, how many are through and how long it takes', async () => {
    const { core, wrapper } = mountPanel();
    await wrapper.get('[data-testid="run"]').trigger('click');
    core.emit(sampleProgress({ estimateMs: 40_000 }));
    await settle();
    const progress = wrapper.get('[data-testid="progress"]');
    expect(progress.text()).toContain('2 of 4 checks');
    expect(progress.text()).toContain('12 s');
    expect(progress.text()).toContain('about 28 s left');
    const running = wrapper.get('[data-testid="progress-a11y-axe"]');
    expect(running.text()).toContain('Running: mobile');
    expect(running.text()).toContain('0/2 views');
    expect(running.text()).not.toContain('tablet');
    const seo = wrapper.get('[data-testid="progress-seo"]').text();
    expect(seo).toContain('2/2 views');
    expect(seo).toContain('1 finding');
    const plugins = wrapper.findAll('[data-testid^="progress-"][data-plugin]').map((row) => row.attributes('data-plugin'));
    // The running check on top.
    expect(plugins).toEqual(['a11y-axe', 'seo']);
    expect(wrapper.find('[data-testid="summary"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('gives no estimate without a last run', async () => {
    const { core, wrapper } = mountPanel();
    core.emit(sampleProgress());
    await settle();
    expect(wrapper.get('[data-testid="progress"]').text()).not.toContain('left');
    wrapper.unmount();
  });

  it('shows the findings of a plugin as soon as it is through and keeps them on cancel', async () => {
    const { core, wrapper } = mountPanel();
    core.emit(sampleProgress());
    core.emit({ type: 'findings', run: sampleRun(), messages: sampleMessages, partial: true });
    await settle();
    expect(wrapper.find('[data-testid="finding-a"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="progress"]').exists()).toBe(true);
    await wrapper.get('[data-testid="cancel"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'runCommand', id: 'checks.cancel' });
    core.emit(
      sampleProgress({
        finished: true,
        cancelled: true,
        jobs: sampleProgress().jobs.map((job) =>
          job.state === 'done' ? job : { ...job, state: 'skipped', reason: 'checks.skip.cancelled' },
        ),
      }),
    );
    core.emit({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    await settle();
    expect(wrapper.find('[data-testid="progress"]').exists()).toBe(false);
    // The run collapses to one line that says it was cancelled.
    expect(wrapper.get('[data-testid="progress-summary"]').text()).toContain('Cancelled: 2 of 4 checks in 12 s');
    expect(wrapper.get('[data-testid="progress-summary"]').text()).not.toContain('skipped');
    expect(wrapper.get('[data-testid="cancelled"]').text()).toContain('cancelled');
    expect(wrapper.find('[data-testid="finding-a"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="run"]').attributes('disabled')).toBeUndefined();
    wrapper.unmount();
  });

  it('names why a plugin failed and in which view', async () => {
    const { core, wrapper } = mountPanel();
    const run = sampleRun();
    run.failures = [{ plugin: 'a11y-axe', view: 2, code: 'plugins.error.timed_out' }, { plugin: 'seo' }];
    core.emit({ type: 'findings', run, messages: sampleMessages });
    await settle();
    const failures = wrapper.findAll('[data-testid="failure"]').map((line) => line.text());
    expect(failures[0]).toBe(
      'The check plugin a11y-axe failed in tablet: The plugin took too long and was stopped. Its findings are missing.',
    );
    expect(failures[1]).toBe('The check plugin seo failed; its findings are missing.');
    wrapper.unmount();
  });

  it('gives a failure its sentence with its parameters, never a raw placeholder', async () => {
    const { core, wrapper } = mountPanel();
    const run = sampleRun();
    run.failures = [{ plugin: 'lighthouse', view: 1, code: 'plugins.error.not_available', params: { service: 'sidecar' } }];
    core.emit({ type: 'findings', run, messages: sampleMessages });
    await settle();
    const failure = wrapper.get('[data-testid="failure"]').text();
    expect(failure).toBe(
      'The check plugin lighthouse failed in mobile: This rBrowser cannot provide sidecar to plugins yet. Its findings are missing.',
    );
    expect(failure).not.toContain('{');
    wrapper.unmount();
  });
  it('names the plugin itself in a failure, never what the plugin put there', async () => {
    const { core, wrapper } = mountPanel();
    const run = sampleRun();
    run.failures = [
      { plugin: 'a11y-axe', view: 2, code: 'plugins.error.crashed', params: { id: 'someone-else', reason: 'TypeError: x' } },
    ];
    core.emit({ type: 'findings', run, messages: sampleMessages });
    await settle();
    const failure = wrapper.get('[data-testid="failure"]').text();
    expect(failure).toContain('The plugin a11y-axe crashed');
    expect(failure).not.toContain('someone-else');
    expect(failure).not.toContain('TypeError');
    wrapper.unmount();
  });

  it("says a plugin's own failure with its reason", async () => {
    const { core, wrapper } = mountPanel();
    const run = sampleRun();
    run.failures = [
      { plugin: 'seo', view: 1, code: 'plugin.seo.errors.no_source', params: { view: '1', reason: 'the page is a data: URL' } },
    ];
    const messages = { ...sampleMessages, 'plugin.seo.errors.no_source': 'View {view} has no served source: {reason}' };
    core.emit({ type: 'findings', run, messages });
    await settle();
    const failure = wrapper.get('[data-testid="failure"]').text();
    expect(failure).toContain('View 1 has no served source: the page is a data: URL');
    wrapper.unmount();
  });

  // RB-099-06: the reason ends its sentence before "Its findings are missing.", with or without
  // a period of its own.
  it("ends the plugin's reason with a period before the closing sentence", async () => {
    const { core, wrapper } = mountPanel();
    const run = sampleRun();
    run.failures = [
      { plugin: 'seo', view: 1, code: 'plugin.seo.errors.no_source', params: { view: '1', reason: 'Resource was not cached' } },
      { plugin: 'seo', view: 2, code: 'plugin.seo.errors.no_source', params: { view: '2', reason: 'Agent is not enabled.' } },
    ];
    const messages = {
      ...sampleMessages,
      'plugin.seo.errors.no_source': 'View {view} has no served source: {reason}',
    };
    core.emit({ type: 'findings', run, messages });
    await settle();
    const texts = wrapper.findAll('[data-testid="failure"]').map((node) => node.text());
    expect(texts.some((text) => text.includes('Resource was not cached. Its findings are missing.'))).toBe(true);
    expect(texts.some((text) => text.includes('Agent is not enabled. Its findings are missing.'))).toBe(true);
    expect(texts.every((text) => !text.includes('..'))).toBe(true);
    wrapper.unmount();
  });
});

describe('FindingsPanel help links', () => {
  it('opens the help page through the core, not as a popup', async () => {
    const { core, wrapper, shellSent } = mountPanel();
    core.emit({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    await settle();
    const help = wrapper.get('[data-testid="help-a"]');
    expect(help.attributes('target')).not.toBe('_blank');
    await help.trigger('click');
    await settle();
    expect(shellSent).toEqual([
      { type: 'runCommand', id: 'app.open_link', args: { url: 'https://help.test/alt' } },
    ]);
    wrapper.unmount();
  });

  it('asks where first when the setting says ask', async () => {
    const { core, wrapper, shellSent } = mountPanel({ 'ui.openLinksIn': 'ask' });
    core.emit({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    await settle();
    await wrapper.get('[data-testid="help-a"]').trigger('click');
    await settle();
    expect(shellSent).toEqual([]);
    const choice = document.body.querySelector<HTMLButtonElement>('[data-testid="link-active-view"]');
    expect(choice).not.toBeNull();
    choice?.click();
    await settle();
    expect(shellSent).toEqual([
      { type: 'runCommand', id: 'app.open_link', args: { url: 'https://help.test/alt', where: 'activeView' } },
    ]);
    wrapper.unmount();
  });
});
