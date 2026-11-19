import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createShellI18n } from '../i18n';
import type { DevicePreset, SetupProject } from '../ipc/protocol';
import { settle } from '../settings/test/mountPage';
import type { SetupApi } from './api';
import ProjectStep from './ProjectStep.vue';

const IPHONE: DevicePreset = {
  id: 'iphone-15',
  kind: 'mobile',
  viewport: { width: 393, height: 852 },
  dpr: 3,
  touch: true,
};

function fakeApi(overrides: Partial<SetupApi> = {}): SetupApi {
  const api: Partial<SetupApi> = {
    devices: vi.fn(async () => [IPHONE]),
    pickFolder: vi.fn(async () => '\\\\wsl.localhost\\Ubuntu\\home\\me\\shop'),
    preview: vi.fn(async (request) => ({
      op: 'proposal' as const,
      requestId: 1,
      proposalId: 5,
      root: request.root,
      config: { version: 1, breakpoints: [{ id: 'mobile', label: 'Mobile', width: 393, height: 852 }] },
      source: 'default' as const,
    })),
    write: vi.fn(async () => ({ op: 'written' as const, requestId: 2, root: 'C:/shop', path: 'C:/shop/.rbrowser/config.json', open: false })),
    runCommand: vi.fn(async () => undefined),
    ...overrides,
  };
  return api as SetupApi;
}

async function mountStep(api: SetupApi, project?: SetupProject) {
  const wrapper = mount(ProjectStep, {
    props: { api, ...(project === undefined ? {} : { project }) },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  await settle();
  return wrapper;
}

afterEach(() => {
  document.body.replaceChildren();
});

describe('the setup wizard project step', () => {
  it('needs a folder first and proposes the open project without config', async () => {
    const empty = await mountStep(fakeApi());
    expect(empty.get('[data-testid="project-preview"]').attributes('disabled')).toBeDefined();
    empty.unmount();
    const open = await mountStep(fakeApi(), { root: 'C:/shop', hasConfig: false });
    expect(open.get('[data-testid="project-root"]').text()).toBe('C:/shop');
    expect(open.get('[data-testid="project-preview"]').attributes('disabled')).toBeUndefined();
    open.unmount();
  });

  it('takes a WSL folder from the folder dialog', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api);
    await wrapper.get('[data-testid="project-pick"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="project-root"]').text()).toBe('\\\\wsl.localhost\\Ubuntu\\home\\me\\shop');
    wrapper.unmount();
  });

  it('shows the preview before anything is written, and writes only on the button', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api, { root: 'C:/shop', hasConfig: false });
    await wrapper.get('[data-testid="project-preview"]').trigger('click');
    await settle();
    expect(api.preview).toHaveBeenCalledWith({ op: 'preview', root: 'C:/shop', template: 'default' });
    expect(api.write).not.toHaveBeenCalled();
    expect(wrapper.find('[data-testid="preview-view-mobile"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="preview-json"]').text()).toContain('"breakpoints"');
    await wrapper.get('[data-testid="project-write"]').trigger('click');
    await settle();
    expect(api.write).toHaveBeenCalledWith(5);
    expect(wrapper.find('[data-testid="project-written"]').exists()).toBe(true);
    expect(wrapper.emitted('written')).toHaveLength(1);
    await wrapper.get('[data-testid="project-open"]').trigger('click');
    expect(api.runCommand).toHaveBeenCalledWith('project.open_recent', { path: 'C:/shop' });
    wrapper.unmount();
  });

  it('asks for the chosen device presets', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api, { root: 'C:/shop', hasConfig: false });
    wrapper.findComponent(URadioGroup).vm.$emit('update:modelValue', 'devices');
    await settle();
    expect(wrapper.get('[data-testid="project-preview"]').attributes('disabled')).toBeDefined();
    await wrapper.get('[data-testid="device-iphone-15"]').trigger('click');
    await settle();
    await wrapper.get('[data-testid="project-preview"]').trigger('click');
    await settle();
    expect(api.preview).toHaveBeenCalledWith({ op: 'preview', root: 'C:/shop', template: 'devices', devices: ['iphone-15'] });
    wrapper.unmount();
  });

  it('names a refusal with its reason and writes nothing', async () => {
    const api = fakeApi({
      preview: vi.fn(async () => ({
        op: 'failed' as const,
        requestId: 1,
        step: 'preview' as const,
        messageKey: 'firstrun.project.error.exists',
        args: { path: 'C:/shop/.rbrowser/config.json' },
      })),
    });
    const wrapper = await mountStep(api, { root: 'C:/shop', hasConfig: false });
    await wrapper.get('[data-testid="project-preview"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="project-failed"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="project-write"]').exists()).toBe(false);
    wrapper.unmount();
  });
});
