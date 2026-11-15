import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import type { StateSnapshot, ViewFrameworks } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import FrameworkDevtoolsOffer from './FrameworkDevtoolsOffer.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

function stateWith(frameworks: ViewFrameworks[], activeView: number | null = 1): StateSnapshot {
  const state = sampleState();
  state.activeView = activeView;
  state.frameworks = frameworks;
  return state;
}

describe('the Framework DevTools offer', () => {
  it('offers the frameworks of the active view and opens the panel on click', async () => {
    const { wrapper, mock } = await mountWithShell(FrameworkDevtoolsOffer, {
      mock: {
        state: stateWith([
          { view: 1, vue: { version: '3.5.22', hook: true, devtools: true }, react: { hook: false, devtools: false } },
          { view: 2, react: { hook: true, devtools: true } },
        ]),
      },
    });
    current = wrapper;
    await settle();
    const vue = wrapper.get('[data-testid="framework-devtools-vue"]');
    expect(vue.text()).toBe('Vue DevTools');
    expect(vue.attributes('title')).toContain('Open the Vue DevTools panel');
    const react = wrapper.get('[data-testid="framework-devtools-react"]');
    expect(react.attributes('title')).toContain('Reload the view once');
    await vue.trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'plugin.framework_devtools.vue' });
    await react.trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'plugin.framework_devtools.react' });
  });

  it('explains a Vue production build without devtools', async () => {
    const { wrapper } = await mountWithShell(FrameworkDevtoolsOffer, {
      mock: { state: stateWith([{ view: 1, vue: { hook: true, devtools: false } }]) },
    });
    current = wrapper;
    await settle();
    expect(wrapper.get('[data-testid="framework-devtools-vue"]').attributes('title')).toContain('__VUE_PROD_DEVTOOLS__');
    expect(wrapper.find('[data-testid="framework-devtools-react"]').exists()).toBe(false);
  });

  it('is gone without a framework in the active view, or without an active view', async () => {
    for (const state of [
      stateWith([{ view: 2, vue: { hook: true, devtools: true } }]),
      stateWith([]),
      stateWith([{ view: 1, vue: { hook: true, devtools: true } }], null),
    ]) {
      const { wrapper } = await mountWithShell(FrameworkDevtoolsOffer, { mock: { state } });
      await settle();
      expect(wrapper.find('[data-testid="framework-devtools-offer"]').exists()).toBe(false);
      wrapper.unmount();
    }
  });
});
