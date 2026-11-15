import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import type { StateSnapshot } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import BrowsersyncNotice from './BrowsersyncNotice.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

function stateWith(enabled: boolean, browsersync?: string): StateSnapshot {
  const state = sampleState();
  state.sync = { ...state.sync, enabled, ...(browsersync === undefined ? {} : { browsersync }) };
  return state;
}

describe('the Browsersync notice', () => {
  it('says why sync is off and syncs anyway on click', async () => {
    const { wrapper, mock } = await mountWithShell(BrowsersyncNotice, {
      mock: { state: stateWith(false, 'http://localhost:3000') },
    });
    current = wrapper;
    await settle();
    const notice = wrapper.get('[data-testid="browsersync-notice"]');
    expect(notice.text()).toContain('Browsersync syncs this page by itself');
    expect(notice.attributes('title')).toContain('http://localhost:3000');
    await wrapper.get('[data-testid="browsersync-anyway"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'setSync', feature: 'all', enabled: true });
  });

  it('is gone while sync is on, and without Browsersync', async () => {
    for (const state of [stateWith(true, 'http://localhost:3000'), stateWith(false)]) {
      const { wrapper } = await mountWithShell(BrowsersyncNotice, { mock: { state } });
      await settle();
      expect(wrapper.find('[data-testid="browsersync-notice"]').exists()).toBe(false);
      wrapper.unmount();
    }
  });
});
