import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../settings/test/mountPage';
import type { SetupApi } from './api';
import BasicsStep from './BasicsStep.vue';

function fakeApi(): SetupApi {
  const api: Partial<SetupApi> = {
    setTheme: vi.fn(async () => undefined),
    basics: vi.fn(async () => ({ op: 'applied' as const, requestId: 1, root: 'C:/shop' })),
  };
  return api as SetupApi;
}

async function mountStep(api: SetupApi, target: string | null) {
  const wrapper = mount(BasicsStep, {
    props: { api, target },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  await settle();
  return wrapper;
}

afterEach(() => {
  document.body.replaceChildren();
});

describe('the setup wizard basics', () => {
  it('applies nothing before the button, then the theme and the project choices', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api, 'C:/shop');
    const [theme, layout] = wrapper.findAllComponents(URadioGroup);
    theme?.vm.$emit('update:modelValue', 'dark');
    layout?.vm.$emit('update:modelValue', 'canvas');
    await settle();
    expect(api.setTheme).not.toHaveBeenCalled();
    await wrapper.get('[data-testid="basics-sync"]').trigger('click');
    await wrapper.get('[data-testid="basics-apply"]').trigger('click');
    await settle();
    expect(api.setTheme).toHaveBeenCalledWith('dark');
    expect(api.basics).toHaveBeenCalledWith('canvas', false);
    expect(wrapper.emitted('applied')?.[0]).toEqual([{ theme: 'dark', layout: 'canvas', sync: false }]);
    wrapper.unmount();
  });

  it('without a project sets only the theme and says why the rest is off', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api, null);
    expect(wrapper.text()).toContain('Layout and sync belong to a project');
    await wrapper.get('[data-testid="basics-apply"]').trigger('click');
    await settle();
    expect(api.setTheme).toHaveBeenCalledWith('system');
    expect(api.basics).not.toHaveBeenCalled();
    expect(wrapper.emitted('applied')?.[0]).toEqual([{ theme: 'system', layout: null, sync: null }]);
    wrapper.unmount();
  });
});
