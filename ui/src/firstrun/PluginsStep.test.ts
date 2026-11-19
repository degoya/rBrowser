import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../settings/test/mountPage';
import type { SetupApi } from './api';
import PluginsStep from './PluginsStep.vue';
import type { InstallOutcome, InstallProgress } from './plugins';
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

  it('installs the choice after one confirmation that names the grants', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api);
    await wrapper.get('[data-testid="plugins-install"]').trigger('click');
    await settle();
    expect(api.installChosen).not.toHaveBeenCalled();
    const confirm = wrapper.get('[data-testid="plugins-confirming"]');
    expect(confirm.text()).toContain('Install 3 first-party plugins?');
    // The grants of the whole choice, named once each.
    expect(confirm.text()).toContain('Signed with');
    await wrapper.get('[data-testid="plugins-confirm"]').trigger('click');
    await settle();
    expect(api.installChosen).toHaveBeenCalledTimes(1);
    expect(vi.mocked(api.installChosen).mock.calls[0]?.[0]).toEqual(['seo', 'axe', 'video']);
    // Nothing installed was unticked: nothing is removed.
    expect(api.removeChosen).not.toHaveBeenCalled();
    expect(wrapper.find('[data-testid="plugins-outcome"]').exists()).toBe(true);
    const emitted = wrapper.emitted('installed') as [InstallOutcome][] | undefined;
    expect(emitted?.[0]?.[0].results.map((result) => result.id)).toEqual(['seo', 'axe', 'video']);
    wrapper.unmount();
  });

  it('names a failed plugin and keeps the others', async () => {
    const api = fakeApi({
      installChosen: async () => ({
        results: [
          { id: 'seo', ok: true, origin: 'repository' },
          { id: 'axe', ok: false, error: 'plugins.catalog.error.unknown_key_x', origin: 'bundle' },
        ],
        restart: true,
      }),
    });
    const wrapper = await mountStep(api);
    await wrapper.get('[data-testid="plugins-install"]').trigger('click');
    await settle();
    await wrapper.get('[data-testid="plugins-confirm"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="plugins-failed-axe"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="plugins-failed-seo"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('removes an installed plugin the person unticked, in the same confirmation', async () => {
    // A choice made before (not pending): what is installed is ticked.
    const api = fakeApi({ available: vi.fn(async () => ({ offline: false, choicePending: false, plugins: SAMPLE_PLUGINS })) });
    const wrapper = await mountStep(api);
    await wrapper.get('[data-testid="plugin-video"]').trigger('click');
    await settle();
    expect(checked(wrapper, 'video')).toBe('false');
    await wrapper.get('[data-testid="plugins-install"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="plugins-confirm-remove"]').exists()).toBe(true);
    expect(api.removeChosen).not.toHaveBeenCalled();
    await wrapper.get('[data-testid="plugins-confirm"]').trigger('click');
    await settle();
    expect(api.installChosen).toHaveBeenCalledTimes(1);
    expect(api.removeChosen).toHaveBeenCalledWith(['video']);
    const emitted = wrapper.emitted('installed') as [{ removed: string[] }][] | undefined;
    expect(emitted?.[0]?.[0].removed).toEqual(['video']);
    wrapper.unmount();
  });
});
