import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { createShellI18n } from '../i18n';
import type { CoreToShell } from '../ipc/protocol';
import WhichKeyBar from './WhichKeyBar.vue';

describe('the which-key overlay', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  // RB-080-18: the overlay stays until the core ends the chord (Esc or a key); the bar runs no
  // timer of its own, so a timeout only ever comes from the core's setting.
  it('stays up while time passes and hides only when the core says so', async () => {
    vi.useFakeTimers();
    let push: (message: CoreToShell) => void = () => undefined;
    const wrapper = mount(WhichKeyBar, {
      props: {
        subscribe: (listener: (message: CoreToShell) => void) => {
          push = listener;
          return () => undefined;
        },
      },
      global: { plugins: [ui, createShellI18n()] },
    });
    push({ type: 'whichKey', typed: 'Ctrl+Shift+Space', entries: [] });
    await nextTick();
    expect(wrapper.find('[data-testid="which-key"]').exists()).toBe(true);
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);
    await nextTick();
    expect(wrapper.find('[data-testid="which-key"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="which-key"]').text()).toContain('Esc cancels');
    push({ type: 'whichKeyHide' });
    await nextTick();
    expect(wrapper.find('[data-testid="which-key"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('shows the next keys after the leader and hides when the chord ends', async () => {
    let push: (message: CoreToShell) => void = () => undefined;
    const wrapper = mount(WhichKeyBar, {
      props: {
        subscribe: (listener: (message: CoreToShell) => void) => {
          push = listener;
          return () => undefined;
        },
      },
      global: { plugins: [ui, createShellI18n()] },
    });
    expect(wrapper.find('[data-testid="which-key"]').exists()).toBe(false);
    // The live region is in the DOM before the leader, so its first content is announced
    // (RB-090-22); the bar comes and goes inside it.
    const region = wrapper.get('[data-testid="which-key-region"]');
    expect(region.attributes('role')).toBe('status');
    expect(region.attributes('aria-label')).toBeTruthy();
    expect(region.text()).toBe('');
    push({
      type: 'whichKey',
      typed: 'Ctrl+Shift+Space',
      entries: [
        { kind: 'group', key: 'S', group: 'sync', titleKey: 'commands.group.sync' },
        { kind: 'command', key: ',', id: 'settings.open', titleKey: 'commands.settings.open' },
      ],
    });
    await nextTick();
    const bar = wrapper.get('[data-testid="which-key"]');
    expect(bar.get('[data-testid="which-key-typed"]').text()).toBe('Ctrl+Shift+Space');
    expect(bar.text()).toContain('Sync');
    expect(bar.findAll('li')).toHaveLength(2);
    expect(wrapper.get('[data-testid="which-key-region"]').element).toBe(region.element);
    expect(region.element.contains(bar.element)).toBe(true);
    expect(bar.attributes('role')).toBeUndefined();
    push({ type: 'whichKeyHide' });
    await nextTick();
    expect(wrapper.find('[data-testid="which-key"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="which-key-region"]').element).toBe(region.element);
  });

  it('listens to the core on the window by default', async () => {
    const wrapper = mount(WhichKeyBar, { global: { plugins: [ui, createShellI18n()] } });
    window.dispatchEvent(
      new CustomEvent('rb-ipc', { detail: { type: 'whichKey', typed: 'Ctrl+Shift+Space', entries: [] } }),
    );
    await nextTick();
    expect(wrapper.find('[data-testid="which-key"]').exists()).toBe(true);
    wrapper.unmount();
  });
});
