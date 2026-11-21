import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import type { McpClientPlan, McpSetupAnswer, ShellToCore } from '../ipc/protocol';
import { settle } from '../settings/test/mountPage';
import SetupWizard from './SetupWizard.vue';
import { fakeCore, SAMPLE_PLUGINS } from './testing';

/** The MCP setup's plan: Claude Code with the project file when there is a project. */
function mcpPlan(requestId: number, root: string | undefined): McpSetupAnswer {
  const claudeCode: McpClientPlan =
    root === undefined
      ? { client: 'claudeCode', projectReason: 'mcp.setup.reason.no_project' }
      : { client: 'claudeCode', project: { path: '.mcp.json', status: 'create', entry: '{}', kept: 0 } };
  return { op: 'plan', requestId, enabled: false, ...(root === undefined ? {} : { root }), clients: [claudeCode] };
}

function core(enabled = true, choicePending = true, root?: string) {
  const fake = fakeCore((message: ShellToCore) => {
    if (message.type === 'setup' && message.request.op === 'info') {
      return { type: 'setup', answer: { op: 'info', enabled, ...(root === undefined ? {} : { project: { root, hasConfig: true } }) } };
    }
    if (message.type === 'mcpSetup') {
      const request = message.request;
      const answer: McpSetupAnswer =
        request.op === 'write'
          ? { op: 'written', requestId: request.requestId, files: [{ path: '.mcp.json', status: 'create' }] }
          : mcpPlan(request.requestId, request.op === 'plan' ? request.root : undefined);
      setTimeout(() => fake.emit({ type: 'mcpSetup', answer }), 0);
      return { type: 'ack' };
    }
    if (message.type === 'pluginPanel' && (message.request as { op: string }).op === 'installChosen') {
      const ids = (message.request as unknown as { ids: string[] }).ids;
      const results = ids.map((id) => ({ id, ok: true, origin: 'bundle' }));
      return { type: 'pluginPanel', message: { value: { results, restart: false } } };
    }
    if (message.type === 'pluginPanel') {
      // A fresh install's pending choice: nothing optional is installed yet.
      const plugins = choicePending ? SAMPLE_PLUGINS.map((plugin) => ({ ...plugin, installed: false })) : SAMPLE_PLUGINS;
      return { type: 'pluginPanel', message: { value: { offline: false, choicePending, plugins } } };
    }
    if (message.type === 'getDevices') {
      return { type: 'devices', items: [] };
    }
    return undefined;
  });
  return fake;
}

const settingWrites = (sent: ShellToCore[]) =>
  sent.flatMap((message) => (message.type === 'setSetting' ? [[message.key, message.value]] : []));

async function mountWizard(fake = core(), routeKey: string | null = null) {
  const wrapper = mount(SetupWizard, {
    props: { client: fake.client, routeKey },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  await settle();
  return wrapper;
}

const pluginOps = (sent: ShellToCore[]) =>
  sent.flatMap((message) => (message.type === 'pluginPanel' ? [(message.request as { op: string }).op] : []));

/** The ids of the first `installChosen`, when one was sent. */
const installIds = (sent: ShellToCore[]) =>
  sent.flatMap((message) =>
    message.type === 'pluginPanel' && (message.request as { op: string }).op === 'installChosen'
      ? [(message.request as unknown as { ids: string[] }).ids]
      : [],
  )[0];

const closes = (sent: ShellToCore[]) =>
  sent.filter((message) => message.type === 'setup' && message.request.op === 'close').length;

afterEach(() => {
  document.body.replaceChildren();
  window.location.hash = '';
});

describe('the setup wizard', () => {
  it('starts with the welcome, and "Just start" closes it without any change', async () => {
    const fake = core();
    const wrapper = await mountWizard(fake);
    expect(wrapper.find('[data-testid="setup-welcome"]').exists()).toBe(true);
    await wrapper.get('[data-testid="setup-just-start"]').trigger('click');
    await settle();
    expect(closes(fake.sent)).toBe(1);
    // Pending was not known yet: asked, then the recommended set applied.
    expect(pluginOps(fake.sent)).toEqual(['available', 'applyRecommended']);
    expect(fake.sent.some((message) => message.type === 'runCommand' || message.type === 'setSetting')).toBe(false);
    wrapper.unmount();
  });

  it('walks every step with its one button, each saying what it does, and ends with the summary', async () => {
    const fake = core();
    const wrapper = await mountWizard(fake);
    await wrapper.get('[data-testid="setup-start"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="setup-plugins"]').exists()).toBe(true);
    const labels: string[] = [];
    for (const next of ['setup-project', 'setup-basics', 'setup-agent', 'setup-done']) {
      labels.push(wrapper.get('[data-testid="setup-next"]').text());
      await wrapper.get('[data-testid="setup-next"]').trigger('click');
      await settle();
      expect(wrapper.find(`[data-testid="${next}"]`).exists()).toBe(true);
    }
    // The recommended plugins (seo, axe, video), no project, the basics, MCP on (RB-091-09).
    expect(labels).toEqual([
      'Install 3 plugins & continue',
      'Continue without a project',
      'Apply & continue',
      'Switch on MCP & continue',
    ]);
    const summary = wrapper.get('[data-testid="setup-summary"]').text();
    expect(summary).toContain('3 plugins installed and loaded.');
    expect(summary).not.toContain('next start');
    expect(wrapper.get('[data-testid="setup-summary"]').findAll('li').length).toBeGreaterThan(1);
    expect(settingWrites(fake.sent)).toEqual(
      expect.arrayContaining([
        ['mcp.client', 'claudeCode'],
        ['mcp.enabled', true],
      ]),
    );
    // No project: no client file was written.
    expect(fake.sent.some((message) => message.type === 'mcpSetup' && message.request.op === 'write')).toBe(false);
    // The ticks were installed, nothing written, and the recommended set never applied behind them.
    expect(pluginOps(fake.sent)).toEqual(['available', 'installChosen']);
    expect(fake.sent.some((message) => message.type === 'setup' && message.request.op === 'write')).toBe(false);
    await wrapper.get('[data-testid="setup-finish"]').trigger('click');
    await settle();
    expect(closes(fake.sent)).toBe(1);
    expect(pluginOps(fake.sent)).not.toContain('applyRecommended');
    wrapper.unmount();
  });

  it('answers a pending choice with nothing when every plugin is unticked', async () => {
    const fake = core();
    const wrapper = await mountWizard(fake);
    await wrapper.get('[data-testid="setup-start"]').trigger('click');
    await settle();
    await wrapper.get('[data-testid="plugins-none"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="setup-next"]').text()).toBe('Continue without plugins');
    await wrapper.get('[data-testid="setup-next"]').trigger('click');
    await settle();
    expect(installIds(fake.sent)).toEqual([]);
    expect(wrapper.find('[data-testid="setup-project"]').exists()).toBe(true);
    await wrapper.get('[data-testid="setup-skip"]').trigger('click');
    await settle();
    // Answered: closing applies nothing more.
    expect(pluginOps(fake.sent)).not.toContain('applyRecommended');
    wrapper.unmount();
  });

  it('passes a step on the stepper without carrying anything out', async () => {
    const fake = core();
    const wrapper = await mountWizard(fake);
    await wrapper.get('[data-testid="setup-start"]').trigger('click');
    await settle();
    await wrapper.get('[data-testid="setup-back"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="setup-welcome"]').exists()).toBe(true);
    expect(pluginOps(fake.sent)).toEqual(['available']);
    wrapper.unmount();
  });

  it('installs the ticked plugins with Next, never the recommended set behind them (RB-091-08)', async () => {
    const fake = core();
    const wrapper = await mountWizard(fake);
    await wrapper.get('[data-testid="setup-start"]').trigger('click');
    await settle();
    // The person unticks a recommended plugin and goes on without any other button.
    await wrapper.get('[data-testid="plugin-seo"]').trigger('click');
    await settle();
    await wrapper.get('[data-testid="setup-next"]').trigger('click');
    await settle();
    expect(installIds(fake.sent)).toEqual(['axe', 'video']);
    expect(pluginOps(fake.sent)).not.toContain('applyRecommended');
    expect(wrapper.find('[data-testid="setup-project"]').exists()).toBe(true);
    wrapper.unmount();
  });

  it('scrolls itself, with Back and Next kept in sight', async () => {
    // The pages window's body never scrolls (assets/main.css); a long plugin list must not hide
    // the footer (owner, 2026-11-18).
    const wrapper = await mountWizard();
    expect(wrapper.get('[data-testid="setup-scroll"]').classes()).toEqual(expect.arrayContaining(['h-screen', 'overflow-y-auto']));
    await wrapper.get('[data-testid="setup-start"]').trigger('click');
    expect(wrapper.get('footer').classes()).toContain('sticky');
  });

  it('offers "Skip setup" on every page', async () => {
    const fake = core();
    const wrapper = await mountWizard(fake);
    await wrapper.get('[data-testid="setup-start"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="setup-skip"]').exists()).toBe(true);
    await wrapper.get('[data-testid="setup-skip"]').trigger('click');
    await settle();
    expect(closes(fake.sent)).toBe(1);
    // Skipped before the plugins were answered: the core installs its recommended set.
    expect(pluginOps(fake.sent)).toContain('applyRecommended');
    wrapper.unmount();
  });

  it('opens at the project step from the hint, and leaves the plugins as they are', async () => {
    const fake = core();
    const wrapper = await mountWizard(fake, 'project');
    expect(wrapper.find('[data-testid="setup-project"]').exists()).toBe(true);
    await wrapper.get('[data-testid="setup-skip"]').trigger('click');
    await settle();
    expect(pluginOps(fake.sent)).toEqual([]);
    wrapper.unmount();
  });

  it('says it is switched off and offers only Close', async () => {
    const wrapper = await mountWizard(core(false));
    expect(wrapper.find('[data-testid="setup-off"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="setup-stepper"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="setup-close"]').exists()).toBe(true);
    wrapper.unmount();
  });

  // RB-094-21: the texts that name Settings > General > Start, and the steps' settings texts, link there.
  it('opens Settings at the key a text names, off, on the welcome and from a step', async () => {
    const opened = (sent: ShellToCore[]) => sent.flatMap((message) => (message.type === 'openSettings' ? [message.key] : []));
    const off = core(false);
    const wrapper = await mountWizard(off);
    await wrapper.get('[data-testid="setup-off-setting"] [data-testid="setting-link-ui.firstRunWizard"]').trigger('click');
    await settle();
    expect(opened(off.sent)).toEqual(['ui.firstRunWizard']);
    wrapper.unmount();
    const on = core();
    const wizard = await mountWizard(on);
    await wizard.get('[data-testid="setup-welcome"] [data-testid="setting-link-ui.firstRunWizard"]').trigger('click');
    await wizard.get('[data-testid="setup-start"]').trigger('click');
    await settle();
    await wizard.get('[data-testid="plugins-intro"] [data-testid="setting-link-plugins.catalog"]').trigger('click');
    await settle();
    expect(opened(on.sent)).toEqual(['ui.firstRunWizard', 'plugins.catalog']);
    wizard.unmount();
  });

  it('opens a place in the settings from the last step', async () => {
    const wrapper = await mountWizard(core());
    await wrapper.get('[data-testid="setup-start"]').trigger('click');
    for (let i = 0; i < 4; i += 1) {
      await settle();
      await wrapper.get('[data-testid="setup-next"]').trigger('click');
    }
    await settle();
    await wrapper.get('[data-testid="setup-where-ui.firstRunWizard"]').trigger('click');
    expect(window.location.hash).toBe('#/settings/ui.firstRunWizard');
    wrapper.unmount();
  });

  it('leaves a choice made before as it is when skipped', async () => {
    const fake = core(true, false);
    const wrapper = await mountWizard(fake);
    await wrapper.get('[data-testid="setup-just-start"]').trigger('click');
    await settle();
    expect(pluginOps(fake.sent)).toEqual(['available']);
    expect(closes(fake.sent)).toBe(1);
    wrapper.unmount();
  });

  it('recommends the AI agent step, which "Skip" passes without switching MCP on (RB-091-09)', async () => {
    const fake = core();
    const wrapper = await mountWizard(fake, 'agent');
    expect(wrapper.find('[data-testid="setup-agent"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="agent-recommended"]').exists()).toBe(true);
    await wrapper.get('[data-testid="agent-skip"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="setup-done"]').exists()).toBe(true);
    expect(settingWrites(fake.sent)).toEqual([]);
    wrapper.unmount();
  });

  it('writes the project file for the open project with the one button (RB-091-09)', async () => {
    const fake = core(true, true, 'C:/site');
    const wrapper = await mountWizard(fake, 'agent');
    await settle();
    expect(wrapper.get('[data-testid="setup-next"]').text()).toBe('Switch on MCP, write 1 file & continue');
    await wrapper.get('[data-testid="setup-next"]').trigger('click');
    // The write's answer, then the plan read again.
    await settle();
    await settle();
    await settle();
    const writes = fake.sent.flatMap((message) => (message.type === 'mcpSetup' && message.request.op === 'write' ? [message.request] : []));
    expect(writes).toEqual([expect.objectContaining({ client: 'claudeCode', root: 'C:/site', project: true, user: false })]);
    expect(wrapper.find('[data-testid="setup-done"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="setup-summary"]').text()).toContain('1 client file written.');
    wrapper.unmount();
  });
});
