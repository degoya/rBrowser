import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../settings/test/mountPage';
import type { SetupApi } from './api';
import PluginsStep from './PluginsStep.vue';
import type { InstallOutcome, InstallProgress } from './plugins';
import type { StepControl } from './steps';
import { SAMPLE_PLUGINS } from './testing';

/** A fresh install: the first choice pending, nothing optional installed yet. */
const PENDING = SAMPLE_PLUGINS.map((plugin) => ({ ...plugin, installed: false }));

function fakeApi(overrides: Partial<SetupApi> = {}): SetupApi {
  const api: Partial<SetupApi> = {
    available: vi.fn(async () => ({ offline: false, choicePending: true, plugins: PENDING })),
    installChosen: vi.fn(async (ids: string[], onProgress: (progress: InstallProgress) => void) => {
      ids.forEach((id, index) => onProgress({ id, done: index + 1, total: ids.length, state: 'installed' }));
      return { results: ids.map((id) => ({ id, ok: true, origin: 'bundle' as const })), restart: true };
    }),
    removeChosen: vi.fn(async (ids: string[]) => ({
      results: ids.map((id) => ({ id, ok: true, origin: 'bundle' as const })),
      restart: true,
    })),
    ...overrides,
  };
  return api as SetupApi;
}

async function mountStep(api: SetupApi) {
  const wrapper = mount(PluginsStep, {
    props: { api },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  await settle();
  return wrapper;
}

// Nuxt UI's checkbox carries the test id on its `role="checkbox"` button (as in the analyser).
const checked = (wrapper: Awaited<ReturnType<typeof mountStep>>, id: string) =>
  wrapper.get(`[data-testid="plugin-${id}"]`).attributes('aria-checked');

/** The wizard's footer button on this step. */
const control = (wrapper: Awaited<ReturnType<typeof mountStep>>) => wrapper.vm as unknown as StepControl;

afterEach(() => {
  document.body.replaceChildren();
});

describe('the setup wizard plugin step', () => {
  it('lists the plugins by category with the recommended ones ticked, installed or not', async () => {
    const wrapper = await mountStep(fakeApi());
    expect(wrapper.find('[data-testid="plugins-group-checks"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="plugins-group-devtools"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="plugins-group-overlays"]').exists()).toBe(false);
    expect(checked(wrapper, 'seo')).toBe('true');
    expect(checked(wrapper, 'vue-devtools')).toBe('false');
    // Recommended: ticked while the first choice is pending, and the person may untick it.
    expect(checked(wrapper, 'video')).toBe('true');
    expect(wrapper.get('[data-testid="plugin-video"]').attributes('disabled')).toBeUndefined();
    expect(wrapper.find('[data-testid="plugins-offline"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('says so without a network', async () => {
    const wrapper = await mountStep(fakeApi({ available: async () => ({ offline: true, choicePending: true, plugins: PENDING }) }));
    expect(wrapper.find('[data-testid="plugins-offline"]').exists()).toBe(true);
    wrapper.unmount();
  });

  it('shows loading, failed with Retry and empty as three states', async () => {
    let fail = true;
    const available = vi.fn(async () => {
      if (fail) throw new Error('down');
      return { offline: false, choicePending: false, plugins: [] };
    });
    const wrapper = await mountStep(fakeApi({ available }));
    expect(wrapper.find('[data-testid="plugins-failed"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="plugins-empty"]').exists()).toBe(false);
    fail = false;
    await wrapper.get('[data-testid="plugins-retry"]').trigger('click');
    await settle();
    expect(available).toHaveBeenCalledTimes(2);
    expect(wrapper.find('[data-testid="plugins-failed"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="plugins-empty"]').exists()).toBe(true);
    wrapper.unmount();
  });

  // RB-094-21: the texts that name Settings > Plugins > Catalog open it.
  it('opens Settings at the catalog from the intro and from a failed listing', async () => {
    const available = vi.fn(async () => {
      throw new Error('down');
    });
    const wrapper = await mountStep(fakeApi({ available }));
    await wrapper.get('[data-testid="plugins-intro"] [data-testid="setting-link-plugins.catalog"]').trigger('click');
    await wrapper.get('[data-testid="plugins-failed"] [data-testid="setting-link-plugins.catalog"]').trigger('click');
    expect(wrapper.emitted('settings')).toEqual([
      [{ type: 'openSettings', key: 'plugins.catalog' }],
      [{ type: 'openSettings', key: 'plugins.catalog' }],
    ]);
    wrapper.unmount();
  });

  it('filters by search and ticks All and None of what is shown', async () => {
    const wrapper = await mountStep(fakeApi());
    await wrapper.get('input[type="search"]').setValue('vue');
    await settle();
    expect(wrapper.find('[data-testid="plugin-seo"]').exists()).toBe(false);
    await wrapper.get('[data-testid="plugins-all"]').trigger('click');
    await settle();
    expect(checked(wrapper, 'vue-devtools')).toBe('true');
    await wrapper.get('[data-testid="plugins-none"]').trigger('click');
    await settle();
    expect(checked(wrapper, 'vue-devtools')).toBe('false');
    wrapper.unmount();
  });

  it('names what the one button does and shows the grants in the step, before any click', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api);
    expect(control(wrapper).next).toEqual({ key: 'firstrun.plugins.install_next', args: { count: 3 }, busy: false });
    // The grants of the whole choice, named once each, stand in the step: the click is the consent.
    expect(wrapper.get('[data-testid="plugins-grants"]').text()).toContain('Signed with');
    expect(api.installChosen).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it('installs the ticks with the button, says they load live, and lets the wizard go on', async () => {
    const api = fakeApi({
      installChosen: vi.fn(async (ids: string[], onProgress: (progress: InstallProgress) => void) => {
        ids.forEach((id, index) => onProgress({ id, done: index + 1, total: ids.length, state: 'installed' }));
        return { results: ids.map((id) => ({ id, ok: true, origin: 'bundle' as const })), restart: false };
      }),
    });
    const wrapper = await mountStep(api);
    expect(await control(wrapper).proceed()).toBe(true);
    await settle();
    expect(vi.mocked(api.installChosen).mock.calls[0]?.[0]).toEqual(['seo', 'axe', 'video']);
    expect(api.removeChosen).not.toHaveBeenCalled();
    expect(wrapper.find('[data-testid="plugins-live"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="plugins-restart"]').exists()).toBe(false);
    const emitted = wrapper.emitted('installed') as [InstallOutcome][] | undefined;
    expect(emitted?.[0]?.[0].results.map((result) => result.id)).toEqual(['seo', 'axe', 'video']);
    // The real state now: installed, nothing left to do.
    expect(control(wrapper).next.key).toBe('firstrun.next');
    wrapper.unmount();
  });

  it('says they load at the next start when the core could not load them live', async () => {
    const wrapper = await mountStep(fakeApi());
    await control(wrapper).proceed();
    await settle();
    expect(wrapper.find('[data-testid="plugins-restart"]').exists()).toBe(true);
    wrapper.unmount();
  });

  it('answers a pending choice with nothing: "Continue without plugins"', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api);
    await wrapper.get('[data-testid="plugins-none"]').trigger('click');
    await settle();
    expect(control(wrapper).next.key).toBe('firstrun.plugins.without_next');
    expect(await control(wrapper).proceed()).toBe(true);
    expect(api.installChosen).toHaveBeenCalledWith([], expect.any(Function));
    wrapper.unmount();
  });

  it('names a failed plugin with Retry, keeps the others, and stays until it is decided', async () => {
    let fail = true;
    const installChosen = vi.fn(async (ids: string[]) => ({
      results: ids.map((id) =>
        id === 'axe' && fail
          ? { id, ok: false, error: 'plugins.catalog.error.unknown_key_x', origin: 'bundle' as const }
          : { id, ok: true, origin: 'repository' as const },
      ),
      restart: false,
    }));
    const api = fakeApi({ installChosen });
    const wrapper = await mountStep(api);
    expect(await control(wrapper).proceed()).toBe(false);
    await settle();
    expect(wrapper.find('[data-testid="plugins-failed-axe"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="plugins-failed-seo"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="plugin-state-axe"]').text()).toBe('Failed');
    // A failure without its own text says "try it again in Settings > Plugins > Catalog" (RB-094-21).
    await wrapper.get('[data-testid="plugins-failed-axe"] [data-testid="setting-link-plugins.catalog"]').trigger('click');
    expect(wrapper.emitted('settings')).toEqual([[{ type: 'openSettings', key: 'plugins.catalog' }]]);
    // The button retries what failed, not what installed.
    expect(control(wrapper).next).toEqual({ key: 'firstrun.plugins.install_next', args: { count: 1 }, busy: false });
    fail = false;
    await wrapper.get('[data-testid="plugins-retry-axe"]').trigger('click');
    await settle();
    expect(installChosen).toHaveBeenLastCalledWith(['axe'], expect.any(Function));
    expect(wrapper.find('[data-testid="plugins-failed-axe"]').exists()).toBe(false);
    expect(await control(wrapper).proceed()).toBe(true);
    wrapper.unmount();
  });

  it('removes an installed plugin the person unticked, said in the step', async () => {
    // A choice made before (not pending): what is installed is ticked.
    const api = fakeApi({ available: vi.fn(async () => ({ offline: false, choicePending: false, plugins: SAMPLE_PLUGINS })) });
    const wrapper = await mountStep(api);
    expect(control(wrapper).next.key).toBe('firstrun.next');
    await wrapper.get('[data-testid="plugin-video"]').trigger('click');
    await settle();
    expect(checked(wrapper, 'video')).toBe('false');
    expect(wrapper.find('[data-testid="plugins-confirm-remove"]').exists()).toBe(true);
    expect(control(wrapper).next).toEqual({ key: 'firstrun.plugins.remove_next', args: { count: 1 }, busy: false });
    expect(api.removeChosen).not.toHaveBeenCalled();
    expect(await control(wrapper).proceed()).toBe(true);
    expect(api.installChosen).toHaveBeenCalledTimes(1);
    expect(api.removeChosen).toHaveBeenCalledWith(['video']);
    const emitted = wrapper.emitted('installed') as [{ removed: string[] }][] | undefined;
    expect(emitted?.[0]?.[0].removed).toEqual(['video']);
    wrapper.unmount();
  });
});
