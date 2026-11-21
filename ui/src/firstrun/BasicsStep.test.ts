import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../settings/test/mountPage';
import type { SetupApi } from './api';
import BasicsStep from './BasicsStep.vue';
import type { StepControl } from './steps';

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

/** The wizard's footer button on this step: its label, and the click. */
const control = (wrapper: Awaited<ReturnType<typeof mountStep>>) => wrapper.vm as unknown as StepControl;

afterEach(() => {
  document.body.replaceChildren();
});

describe('the setup wizard basics', () => {
  it('applies nothing before "Apply & continue", then the theme and the project choices', async () => {
    const api = fakeApi();
    const wrapper = await mountStep(api, 'C:/shop');
    const [theme, layout] = wrapper.findAllComponents(URadioGroup);
    theme?.vm.$emit('update:modelValue', 'dark');
    layout?.vm.$emit('update:modelValue', 'canvas');
    await settle();
    expect(api.setTheme).not.toHaveBeenCalled();
    await wrapper.get('[data-testid="basics-sync"]').trigger('click');
    expect(control(wrapper).next.key).toBe('firstrun.basics.apply_next');
    expect(await control(wrapper).proceed()).toBe(true);
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
    expect(await control(wrapper).proceed()).toBe(true);
    await settle();
    expect(api.setTheme).toHaveBeenCalledWith('system');
    expect(api.basics).not.toHaveBeenCalled();
    expect(wrapper.emitted('applied')?.[0]).toEqual([{ theme: 'system', layout: null, sync: null }]);
    wrapper.unmount();
  });

  it('stays on a failure and says why', async () => {
    const api = fakeApi();
    vi.mocked(api.basics).mockResolvedValueOnce({
      op: 'failed',
      requestId: 1,
      step: 'basics',
      messageKey: 'firstrun.basics.error.no_project',
    });
    const wrapper = await mountStep(api, 'C:/shop');
    expect(await control(wrapper).proceed()).toBe(false);
    await settle();
    expect(wrapper.find('[data-testid="basics-failed"]').exists()).toBe(true);
    expect(wrapper.emitted('applied')).toBeUndefined();
    wrapper.unmount();
  });

  // RB-090-08: a radio group in a form field is no labelable element, so the field's label does
  // not name it; the group carries the label's text itself.
  it('names its radio groups with their field labels', async () => {
    const wrapper = await mountStep(fakeApi(), 'C:/shop');
    const names = wrapper.findAll('[role="radiogroup"]').map((group) => group.attributes('aria-label'));
    expect(names).toEqual(['Theme', 'Layout']);
    wrapper.unmount();
  });
});
