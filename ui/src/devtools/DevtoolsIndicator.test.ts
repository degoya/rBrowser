import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import { mountWithShell, settle } from '../test/mountShell';
import DevtoolsIndicator from './DevtoolsIndicator.vue';
import { devtoolsIndicator, MODE_KEYS } from './indicator';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

describe('the DevTools indicator', () => {
  it('names the inspected view and the mode, or nothing while DevTools are closed', () => {
    const state = sampleState();
    expect(devtoolsIndicator(null)).toBeNull();
    expect(devtoolsIndicator(state)).toBeNull();
    state.devtools = { view: 2, mode: 'docked-right' };
    expect(devtoolsIndicator(state)).toEqual({
      view: 'Tablet',
      modeKey: 'shell.devtools.mode.docked_right',
    });
    state.devtools = { view: 9, mode: 'window' };
    expect(devtoolsIndicator(state)).toEqual({ view: '#9', modeKey: MODE_KEYS.window });
  });

  it('is hidden while DevTools are closed', async () => {
    const { wrapper } = await mountWithShell(DevtoolsIndicator);
    current = wrapper;
    await settle();
    expect(wrapper.find('[data-testid="devtools-indicator"]').exists()).toBe(false);
  });

  it('shows the inspected view and closes DevTools on click', async () => {
    const state = { ...sampleState(), devtools: { view: 1, mode: 'docked-bottom' as const } };
    const { wrapper, mock } = await mountWithShell(DevtoolsIndicator, { mock: { state } });
    current = wrapper;
    await settle();
    const button = wrapper.get('[data-testid="devtools-indicator"]');
    expect(button.text()).toBe('DevTools: Mobile');
    // Label in name (RB-090-23, WCAG 2.5.3): the visible text is the name, the title describes.
    expect(button.attributes('aria-label')).toBeUndefined();
    expect(button.attributes('title')).toBe('DevTools inspect Mobile, docked below. Click to close them.');
    await button.trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'devtools.toggle' });
  });
});
