import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createShellI18n } from '../i18n';
import type { DevicePreset, SetupProject } from '../ipc/protocol';
import { settle } from '../settings/test/mountPage';
import SuggestInput from '../settings/SuggestInput.vue';
import type { SetupApi } from './api';
import ProjectStep from './ProjectStep.vue';
import type { StepControl } from './steps';
import { unnamedFocusables } from '../test/keyboard';

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
    // The preview runs as soon as the input rests: at once in the tests.
    props: { api, debounceMs: 0, ...(project === undefined ? {} : { project }) },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  await settle();
  return wrapper;
}

/** The wizard's footer button on this step. */
const control = (wrapper: Awaited<ReturnType<typeof mountStep>>) => wrapper.vm as unknown as StepControl;

async function typeUrl(wrapper: Awaited<ReturnType<typeof mountStep>>, url: string) {
  wrapper.findComponent(SuggestInput).vm.$emit('commit', url);
  await settle();
}

const ANALYSED = {
  version: 1,
  startUrl: 'http://localhost:5173',
  breakpoints: [
    { id: 'sm', label: 'sm', width: 640, height: 900 },
    { id: 'lg', label: 'lg', width: 1024, height: 900 },
  ],
};

afterEach(() => {
  document.body.replaceChildren();
});

describe('the setup wizard project step', () => {
  it('needs a folder first, and goes on without a project then', async () => {
    const api = fakeApi();
    const empty = await mountStep(api);
    expect(empty.find('[data-testid="project-pick-first"]').exists()).toBe(true);
    expect(control(empty).next.key).toBe('firstrun.project.skip_next');
    expect(await control(empty).proceed()).toBe(true);
    expect(api.preview).not.toHaveBeenCalled();
    expect(api.write).not.toHaveBeenCalled();
    empty.unmount();
  });

  it('takes a WSL folder from the folder dialog and previews it by itself', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api);
    await wrapper.get('[data-testid="project-pick"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="project-root"]').text()).toBe('\\\\wsl.localhost\\Ubuntu\\home\\me\\shop');
    expect(api.preview).toHaveBeenCalledWith({ op: 'preview', root: '\\\\wsl.localhost\\Ubuntu\\home\\me\\shop', template: 'default' });
    wrapper.unmount();
  });

  it('shows the preview of the open project before anything is written; "Write config & continue" writes it', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api, { root: 'C:/shop', hasConfig: false });
    expect(api.preview).toHaveBeenCalledWith({ op: 'preview', root: 'C:/shop', template: 'default' });
    expect(api.write).not.toHaveBeenCalled();
    expect(wrapper.find('[data-testid="preview-view-mobile"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="preview-json"]').text()).toContain('"breakpoints"');
    // RB-090-08: the scrolling preview is a Tab stop and so a named region.
    expect(wrapper.get('[data-testid="preview-json"]').attributes('role')).toBe('region');
    expect(unnamedFocusables(wrapper.element)).toEqual([]);
    expect(control(wrapper).next.key).toBe('firstrun.project.write_next');
    expect(await control(wrapper).proceed()).toBe(true);
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
    vi.mocked(api.preview).mockClear();
    wrapper.findComponent(URadioGroup).vm.$emit('update:modelValue', 'devices');
    await settle();
    // No device yet: nothing to preview, and nothing to write.
    expect(api.preview).not.toHaveBeenCalled();
    expect(control(wrapper).next.key).toBe('firstrun.project.skip_next');
    await wrapper.get('[data-testid="device-iphone-15"]').trigger('click');
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
    expect(wrapper.find('[data-testid="project-failed"]').exists()).toBe(true);
    expect(control(wrapper).next.key).toBe('firstrun.project.skip_next');
    expect(await control(wrapper).proceed()).toBe(true);
    expect(api.write).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it('preselects the analyser with an address, runs it by itself and says what it found', async () => {
    const api = fakeApi({
      preview: vi.fn(async (request) => ({
        op: 'proposal' as const,
        requestId: 1,
        proposalId: 6,
        root: request.root,
        config: ANALYSED,
        source: 'analyzer' as const,
        sheets: 3,
      })),
    });
    const wrapper = await mountStep(api, { root: 'C:/shop', hasConfig: false });
    vi.mocked(api.preview).mockClear();
    await typeUrl(wrapper, 'http://localhost:5173');
    expect(api.preview).toHaveBeenCalledWith({ op: 'preview', root: 'C:/shop', template: 'analyzer', url: 'http://localhost:5173' });
    expect(wrapper.get('[data-testid="project-found"]').text()).toContain('2 breakpoints in 3 stylesheets');
    expect(control(wrapper).next.key).toBe('firstrun.project.write_next');
    expect(await control(wrapper).proceed()).toBe(true);
    expect(api.write).toHaveBeenCalledWith(6);
    wrapper.unmount();
  });

  it('names why the analyser had no proposal, never writes its fallback unasked, and offers Try again and mobile + desktop', async () => {
    const preview = vi.fn(async (request: { root: string; template: string }) => ({
      op: 'proposal' as const,
      requestId: 1,
      proposalId: request.template === 'analyzer' ? 7 : 8,
      root: request.root,
      config: { version: 1, breakpoints: [{ id: 'mobile', label: 'Mobile', width: 393, height: 852 }] },
      source: 'default' as const,
      ...(request.template === 'analyzer' ? { reason: { code: 'unreachable' as const } } : {}),
    }));
    const api = fakeApi({ preview });
    const wrapper = await mountStep(api, { root: 'C:/shop', hasConfig: false });
    await typeUrl(wrapper, 'http://localhost:5999');
    const failed = wrapper.get('[data-testid="project-analyzer-failed"]');
    expect(failed.text()).toContain('http://localhost:5999 could not be reached');
    // Not the analyser's proposal: no preview to write, the button goes on without a project.
    expect(wrapper.find('[data-testid="preview-json"]').exists()).toBe(false);
    expect(control(wrapper).next.key).toBe('firstrun.project.skip_next');
    preview.mockClear();
    await wrapper.get('[data-testid="project-retry"]').trigger('click');
    await settle();
    expect(preview).toHaveBeenCalledWith({ op: 'preview', root: 'C:/shop', template: 'analyzer', url: 'http://localhost:5999' });
    await wrapper.get('[data-testid="project-use-default"]').trigger('click');
    await settle();
    expect(preview).toHaveBeenLastCalledWith({ op: 'preview', root: 'C:/shop', template: 'default', url: 'http://localhost:5999' });
    expect(wrapper.find('[data-testid="project-analyzer-failed"]').exists()).toBe(false);
    expect(control(wrapper).next.key).toBe('firstrun.project.write_next');
    expect(await control(wrapper).proceed()).toBe(true);
    expect(api.write).toHaveBeenCalledWith(8);
    wrapper.unmount();
  });

  // RB-094-21: "install it later in Settings > Plugins > Catalog" opens the catalog.
  it('links to the plugin catalog when the analyser plugin is not loaded', async () => {
    let missing = true;
    const preview = vi.fn(async (request: { root: string; template: string }) => ({
      op: 'proposal' as const,
      requestId: 1,
      proposalId: 7,
      root: request.root,
      config: { version: 1, breakpoints: [{ id: 'mobile', label: 'Mobile', width: 393, height: 852 }] },
      source: 'default' as const,
      reason: { code: request.template === 'analyzer' && missing ? ('plugin_missing' as const) : ('unreachable' as const) },
    }));
    const wrapper = await mountStep(fakeApi({ preview }), { root: 'C:/shop', hasConfig: false });
    await typeUrl(wrapper, 'http://localhost:5999');
    await wrapper.get('[data-testid="project-analyzer-failed"] [data-testid="setting-link-plugins.catalog"]').trigger('click');
    expect(wrapper.emitted('settings')).toEqual([[{ type: 'openSettings', key: 'plugins.catalog' }]]);
    missing = false;
    await wrapper.get('[data-testid="project-retry"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="setting-link-plugins.catalog"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('keeps a template the person picked when the address changes', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api, { root: 'C:/shop', hasConfig: false });
    wrapper.findComponent(URadioGroup).vm.$emit('update:modelValue', 'default');
    await settle();
    vi.mocked(api.preview).mockClear();
    await typeUrl(wrapper, 'http://localhost:3000');
    expect(api.preview).toHaveBeenCalledWith({ op: 'preview', root: 'C:/shop', template: 'default', url: 'http://localhost:3000' });
    wrapper.unmount();
  });

  // RB-090-08: the template group carries its field's label as its name (no labelable element).
  it('names the template radio group with its field label', async () => {
    const wrapper = await mountStep(fakeApi(), { root: 'C:/shop', hasConfig: false });
    expect(wrapper.get('[role="radiogroup"]').attributes('aria-label')).toBe('Breakpoints');
    wrapper.unmount();
  });
});
