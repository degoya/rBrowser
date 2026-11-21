import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import type { McpSetupAnswer, ShellToCore } from '../ipc/protocol';
import { fakeCore } from '../firstrun/testing';
import { settle } from '../settings/test/mountPage';
import McpSettings from './McpSettings.vue';

function core(answer: (request: Extract<ShellToCore, { type: 'mcpSetup' }>['request']) => McpSetupAnswer) {
  const fake = fakeCore((message) => {
    if (message.type !== 'mcpSetup') return undefined;
    setTimeout(() => fake.emit({ type: 'mcpSetup', answer: answer(message.request) }), 0);
    return { type: 'ack' };
  });
  return fake;
}

const plan = (requestId: number, status: 'create' | 'unchanged'): McpSetupAnswer => ({
  op: 'plan',
  requestId,
  enabled: true,
  root: 'C:/shop',
  clients: [
    { client: 'claudeCode', project: { path: '.mcp.json', status: 'create', entry: '{}', kept: 0 } },
    {
      client: 'cursor',
      project: { path: '.cursor/mcp.json', status, entry: '{}', kept: 0 },
      userFile: { path: 'C:\\Users\\me\\.cursor\\mcp.json', status: 'add', entry: '{}', kept: 1 },
    },
  ],
});

afterEach(() => {
  document.body.replaceChildren();
});

describe('Settings > Automation > MCP', () => {
  it('shows the client the setting names and writes its ticks, the user file with a backup', async () => {
    let written = false;
    const fake = core((request) => {
      if (request.op === 'write') {
        written = true;
        return {
          op: 'written',
          requestId: request.requestId,
          files: [
            { path: '.cursor/mcp.json', status: 'create' },
            { path: 'C:\\Users\\me\\.cursor\\mcp.json', status: 'add', backup: 'C:\\Users\\me\\.cursor\\mcp.json.rbrowser-1.bak' },
          ],
        };
      }
      return plan(request.requestId, written ? 'unchanged' : 'create');
    });
    const wrapper = mount(McpSettings, {
      props: { client: fake.client, selected: 'cursor' },
      attachTo: document.body,
      global: { plugins: [ui, createShellI18n()] },
    });
    await settle();
    await settle();
    expect(wrapper.find('[data-testid="mcp-connect-cursor"]').exists()).toBe(true);
    await wrapper.get('[data-testid="mcp-write-user"]').trigger('click');
    await settle();
    await wrapper.get('[data-testid="mcp-apply"]').trigger('click');
    await settle();
    await settle();
    await settle();
    const writes = fake.sent.flatMap((message) => (message.type === 'mcpSetup' && message.request.op === 'write' ? [message.request] : []));
    expect(writes).toEqual([expect.objectContaining({ client: 'cursor', project: true, user: true })]);
    expect(writes[0]).not.toHaveProperty('root');
    expect(wrapper.get('[data-testid="mcp-backups"]').text()).toContain('rbrowser-1.bak');
    // The server's switch is never touched from here.
    expect(fake.sent.some((message) => message.type === 'setSetting')).toBe(false);
    wrapper.unmount();
  });

  it('writes nothing while nothing is ticked', async () => {
    const fake = core((request) => plan(request.requestId, 'unchanged'));
    const wrapper = mount(McpSettings, {
      props: { client: fake.client, selected: 'cursor' },
      attachTo: document.body,
      global: { plugins: [ui, createShellI18n()] },
    });
    await settle();
    await settle();
    expect(wrapper.get('[data-testid="mcp-apply"]').attributes('disabled')).toBeDefined();
    wrapper.unmount();
  });
});
