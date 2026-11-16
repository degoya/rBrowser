import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import type { StateSnapshot } from '../ipc/protocol';
import CellLabelApp from '../cells/CellLabelApp.vue';
import { mountWithShell, settle } from '../test/mountShell';
import ViewAddressBar from './ViewAddressBar.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

describe('the address field of a decoupled view', () => {
  it('loads a completed address on Enter and restores on Escape', async () => {
    const { wrapper } = await mountWithShell(ViewAddressBar, {
      mount: { props: { url: 'http://localhost:5173/', label: 'Mobile' } },
    });
    current = wrapper;
    const input = wrapper.get('[data-testid="view-address-input"]');
    expect((input.element as HTMLInputElement).value).toBe('http://localhost:5173/');
    expect(wrapper.get('label').text()).toBe('Address of Mobile');
    await input.setValue('localhost:5173/settings');
    await wrapper.get('form').trigger('submit');
    expect(wrapper.emitted('navigate')).toEqual([['http://localhost:5173/settings']]);
    await input.setValue('somewhere-else');
    await input.trigger('keydown', { key: 'Escape' });
    // The view's address until its navigation commits.
    expect((input.element as HTMLInputElement).value).toBe('http://localhost:5173/');
  });

  it('refuses a script address and marks the field', async () => {
    const { wrapper } = await mountWithShell(ViewAddressBar, {
      mount: { props: { url: 'http://localhost:5173/', label: 'Mobile' } },
    });
    current = wrapper;
    const input = wrapper.get('[data-testid="view-address-input"]');
    await input.setValue('javascript:alert(1)');
    await wrapper.get('form').trigger('submit');
    expect(wrapper.emitted('navigate')).toBeUndefined();
    expect(input.attributes('aria-invalid')).toBe('true');
  });

  it('replaces nothing in a coupled view and navigates only its own view when decoupled', async () => {
    const coupled = await mountWithShell(CellLabelApp, { mount: { props: { viewId: 3 } } });
    current = coupled.wrapper;
    await settle();
    expect(coupled.wrapper.find('[data-testid="view-address"]').exists()).toBe(false);
    coupled.wrapper.unmount();

    const state: StateSnapshot = sampleState();
    state.sync = { ...state.sync, navigationDecoupled: [3] };
    const { wrapper, mock } = await mountWithShell(CellLabelApp, {
      mock: { state },
      mount: { props: { viewId: 3 } },
    });
    current = wrapper;
    await settle();
    const input = wrapper.get('[data-testid="view-address-input"]');
    await input.setValue('http://localhost:5173/m/settings');
    await wrapper.get('[data-testid="view-address"]').trigger('submit');
    await settle();
    expect(mock.sent.at(-1)).toEqual({
      type: 'runCommand',
      id: 'view.navigate',
      args: { url: 'http://localhost:5173/m/settings', view: 3 },
    });
  });
});
