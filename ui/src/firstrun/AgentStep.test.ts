import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import USelect from '@nuxt/ui/components/Select.vue';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createShellI18n } from '../i18n';
import type { McpClientPlan, McpSetupAnswer } from '../ipc/protocol';
import type { McpSetupApi } from '../mcpsetup/api';
import { settle } from '../settings/test/mountPage';
import AgentStep from './AgentStep.vue';
import type { StepControl } from './steps';

const PLANS: McpClientPlan[] = [
  {
    client: 'claudeCode',
    project: { path: '.mcp.json', status: 'add', entry: '{\n  "command": "rbrowser.exe"\n}', kept: 2 },
    commands: [
      { shell: 'windows', line: 'claude mcp add --scope user rbrowser -- "C:\\rbrowser.exe" mcp' },
      { shell: 'wsl', line: 'claude mcp add --scope user rbrowser -- "/mnt/c/rbrowser.exe" mcp' },
    ],
  },
  {
    client: 'claudeDesktop',
    projectReason: 'mcp.setup.reason.no_project_file',
    userFile: { path: 'C:\\Users\\me\\AppData\\Roaming\\Claude\\claude_desktop_config.json', status: 'create', entry: '{}', kept: 0 },
    note: 'mcp.setup.note.claude_desktop',
  },
];

function fakeApi(overrides: Record<string, unknown> = {}): McpSetupApi {
  return {
    plan: vi.fn(async (): Promise<McpSetupAnswer> => ({ op: 'plan', requestId: 1, enabled: false, root: 'C:/shop', clients: PLANS })),
    write: vi.fn(async (): Promise<McpSetupAnswer> => ({ op: 'written', requestId: 2, files: [{ path: '.mcp.json', status: 'add' }] })),
    check: vi.fn(async (): Promise<McpSetupAnswer> => ({ op: 'check', requestId: 3, enabled: true, running: true, port: 7420, lastCallSecs: 4, calls: 2 })),
    enable: vi.fn(async () => true),
    setClient: vi.fn(async () => undefined),
    ...overrides,
  } as unknown as McpSetupApi;
}

async function mountStep(api: McpSetupApi, root: string | null = 'C:/shop') {
  const wrapper = mount(AgentStep, { props: { api, root }, attachTo: document.body, global: { plugins: [ui, createShellI18n()] } });
  await settle();
  return wrapper;
}

const control = (wrapper: Awaited<ReturnType<typeof mountStep>>) => wrapper.vm as unknown as StepControl;

afterEach(() => {
  document.body.replaceChildren();
});

describe('the setup wizard AI agent step', () => {
  it('shows both ways for the client, the project file ticked, and says what stays protected', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api);
    expect(api.plan).toHaveBeenCalledWith('C:/shop');
    expect(wrapper.get('[data-testid="mcp-project-entry"]').text()).toContain('rbrowser.exe');
    expect(wrapper.get('[data-testid="mcp-project-status"]').text()).toContain('2 other servers');
    expect(wrapper.get('[data-testid="mcp-command-windows"]').text()).toContain('--scope user');
    expect(wrapper.get('[data-testid="mcp-command-wsl"]').text()).toContain('/mnt/c/');
    expect(wrapper.text()).toContain('MCP cannot change them');
    expect(control(wrapper).next).toMatchObject({ key: 'firstrun.mcp.next_write', args: { count: 1 } });
    wrapper.unmount();
  });

  it('switches MCP on, remembers the client and writes the project file with the one button', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api);
    expect(await control(wrapper).proceed()).toBe(true);
    expect(api.setClient).toHaveBeenCalledWith('claudeCode');
    expect(api.enable).toHaveBeenCalledOnce();
    expect(api.write).toHaveBeenCalledWith({ client: 'claudeCode', root: 'C:/shop', project: true, user: false });
    expect(wrapper.emitted('connected')?.[0]).toEqual([{ client: 'claudeCode', files: 1 }]);
    wrapper.unmount();
  });

  it('writes a user file only when the person ticks its confirmation', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api);
    wrapper.findComponent(USelect).vm.$emit('update:modelValue', 'claudeDesktop');
    await settle();
    expect(wrapper.get('[data-testid="mcp-project-reason"]').text()).toContain('no project configuration');
    // Nothing ticked by itself: only MCP is switched on.
    expect(control(wrapper).next).toMatchObject({ key: 'firstrun.mcp.next' });
    await wrapper.get('[data-testid="mcp-write-user"]').trigger('click');
    await settle();
    expect(control(wrapper).next).toMatchObject({ key: 'firstrun.mcp.next_write', args: { count: 1 } });
    expect(await control(wrapper).proceed()).toBe(true);
    expect(api.write).toHaveBeenCalledWith({ client: 'claudeDesktop', root: 'C:/shop', project: false, user: true });
    wrapper.unmount();
  });

  it('stays on the step with the reason when the write fails', async () => {
    const api = fakeApi({
      write: vi.fn(async (): Promise<McpSetupAnswer> => ({
        op: 'failed',
        requestId: 2,
        messageKey: 'mcp.setup.error.unreadable',
        args: { path: '.mcp.json' },
      })),
    });
    const wrapper = await mountStep(api);
    expect(await control(wrapper).proceed()).toBe(false);
    await settle();
    expect(wrapper.get('[data-testid="agent-failed"]').text()).toContain('.mcp.json');
    expect(wrapper.emitted('connected')).toBeUndefined();
    wrapper.unmount();
  });

  it('does not go on when MCP could not be switched on', async () => {
    const api = fakeApi({ enable: vi.fn(async () => false) });
    const wrapper = await mountStep(api);
    expect(await control(wrapper).proceed()).toBe(false);
    expect(api.write).not.toHaveBeenCalled();
    // RB-094-21: "switch it on in Settings > Automation > MCP" opens there.
    await settle();
    await wrapper.get('[data-testid="agent-failed-setting"] [data-testid="setting-link-mcp.enabled"]').trigger('click');
    expect(wrapper.emitted('settings')).toEqual([[{ type: 'openSettings', key: 'mcp.enabled' }]]);
    wrapper.unmount();
  });

  it('shows no settings link for a failure that names no setting', async () => {
    const api = fakeApi({
      write: vi.fn(async (): Promise<McpSetupAnswer> => ({
        op: 'failed',
        requestId: 2,
        messageKey: 'mcp.setup.error.unreadable',
        args: { path: '.mcp.json' },
      })),
    });
    const wrapper = await mountStep(api);
    expect(await control(wrapper).proceed()).toBe(false);
    await settle();
    expect(wrapper.find('[data-testid="agent-failed-setting"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('checks the connection without a token, and skips on "Skip"', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api);
    await wrapper.get('[data-testid="mcp-check"]').trigger('click');
    await settle();
    const result = wrapper.get('[data-testid="mcp-check-result"]').text();
    expect(result).toContain('4');
    expect(result).not.toMatch(/token/i);
    await wrapper.get('[data-testid="agent-skip"]').trigger('click');
    expect(wrapper.emitted('skip')).toHaveLength(1);
    expect(api.enable).not.toHaveBeenCalled();
    wrapper.unmount();
  });
});
