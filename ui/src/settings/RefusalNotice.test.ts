import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import RefusalNotice from './RefusalNotice.vue';

afterEach(() => {
  document.body.replaceChildren();
});

// RB-094-25 (owner): a Delete in Settings > Breakpoints that the core refused changed nothing on
// screen - the reasons sat under the list, out of sight. The refusal is said at the top.
describe('the notice of a refused write', () => {
  const refusal = {
    key: 'breakpoints',
    issues: [
      { path: '/layout/cells/3/breakpoint', messageKey: 'config.error.unknown_breakpoint' },
      { path: '', messageKey: 'settings.rejected.not_saved' },
    ],
  };

  it('names the setting and every reason as an alert', () => {
    const wrapper = mount(RefusalNotice, {
      props: { refusal, label: 'Breakpoints' },
      attachTo: document.body,
      global: { plugins: [ui, createShellI18n()] },
    });
    const notice = wrapper.get('[data-testid="refusal-notice"]');
    expect(notice.attributes('role')).toBe('alert');
    expect(notice.text()).toContain('Breakpoints was not saved');
    const reasons = wrapper.findAll('[data-testid="refusal-reason"]').map((reason) => reason.text());
    expect(reasons).toEqual([
      '/layout/cells/3/breakpoint: No breakpoint has this id.',
      'The settings file could not be written; nothing was changed.',
    ]);
  });

  it('opens the setting and closes on request', async () => {
    const wrapper = mount(RefusalNotice, {
      props: { refusal, label: 'Breakpoints' },
      attachTo: document.body,
      global: { plugins: [ui, createShellI18n()] },
    });
    await wrapper.get('[data-testid="refusal-show"]').trigger('click');
    expect(wrapper.emitted('show')).toEqual([['breakpoints']]);
    const close = wrapper.get('[data-testid="refusal-close"]');
    expect(close.attributes('aria-label')).toBe('Close the notice');
    await close.trigger('click');
    expect(wrapper.emitted('dismiss')).toHaveLength(1);
  });
});
