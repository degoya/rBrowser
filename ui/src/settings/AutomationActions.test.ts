// RB-090-63: the CDP proxy and MCP sections of Settings > Automation run their commands - the
// endpoint, the Playwright script, a new token, the Claude Code line - from a button beside the
// keys, as the main menu does.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import AutomationActions from './AutomationActions.vue';
import { createFakeCore } from './test/fakeCore';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

async function open(switchKey: string, enabled = true) {
  const core = createFakeCore();
  const mounted = await mountPage(AutomationActions, { switchKey, enabled }, core);
  current = mounted.wrapper;
  return mounted;
}

describe('automation actions', () => {
  it('runs the CDP proxy commands from its section', async () => {
    const { wrapper, core } = await open('cdp.enabled');
    const ids = ['cdp.copy_endpoint', 'cdp.copy_snippet', 'cdp.rotate_token'];
    for (const id of ids) {
      await wrapper.get(`[data-testid="automation-${id}"]`).trigger('click');
      await settle();
      expect(core.sent).toContainEqual({ type: 'runCommand', id });
    }
    expect(wrapper.find('[data-testid="automation-mcp.copy_config"]').exists()).toBe(false);
  });

  it('runs the update check and the install from General > Updates (RB-090-14)', async () => {
    const { wrapper, core } = await open('updates.enabled');
    for (const id of ['update.check', 'update.install']) {
      await wrapper.get(`[data-testid="automation-${id}"]`).trigger('click');
      await settle();
      expect(core.sent).toContainEqual({ type: 'runCommand', id });
    }
    expect(wrapper.findAll('button')).toHaveLength(2);
  });

  it('runs no update command while self-update is off', async () => {
    const { wrapper, core } = await open('updates.enabled', false);
    await wrapper.get('[data-testid="automation-update.install"]').trigger('click');
    await settle();
    expect(core.sent).not.toContainEqual({ type: 'runCommand', id: 'update.install' });
    expect(wrapper.find('[data-testid="automation-off"]').exists()).toBe(true);
  });

  it('shows nothing under any other switch', async () => {
    const { wrapper } = await open('sync.enabled');
    expect(wrapper.findAll('button')).toHaveLength(0);
  });

  it('runs the Claude Code line from the MCP section', async () => {
    const { wrapper, core } = await open('mcp.enabled');
    await wrapper.get('[data-testid="automation-mcp.copy_config"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'runCommand', id: 'mcp.copy_config' });
    expect(wrapper.find('[data-testid="automation-cdp.copy_endpoint"]').exists()).toBe(false);
  });

  it('names each icon button by its command, the tooltip too', async () => {
    const { wrapper } = await open('cdp.enabled');
    const button = wrapper.get('[data-testid="automation-cdp.rotate_token"]');
    expect(button.attributes('aria-label')).toBe('Give the CDP proxy a new token');
    expect(button.attributes('title')).toBe('Give the CDP proxy a new token');
    expect(wrapper.find('[data-testid="automation-off"]').exists()).toBe(false);
  });

  it('is disabled while the service is off, and says so', async () => {
    const { wrapper, core } = await open('mcp.enabled', false);
    const button = wrapper.get('[data-testid="automation-mcp.copy_config"]');
    expect(button.attributes('disabled')).toBeDefined();
    expect(wrapper.get('[data-testid="automation-off"]').text()).toBe('Switch it on above to use these.');
    await button.trigger('click');
    await settle();
    expect(core.sent.filter((message) => message.type === 'runCommand')).toEqual([]);
  });

  it('opens the setup wizard again from Settings > General > Start (RB-091-05)', async () => {
    const { wrapper, core } = await open('ui.firstRunWizard');
    await wrapper.get('[data-testid="automation-app.setup_wizard"]').trigger('click');
    await settle();
    expect(core.sent.filter((message) => message.type === 'runCommand')).toEqual([
      { type: 'runCommand', id: 'app.setup_wizard' },
    ]);
  });
});
