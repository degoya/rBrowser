import { mount } from '@vue/test-utils';
import ui from '@nuxt/ui/vue-plugin';
import { describe, expect, it, vi } from 'vitest';
import { h } from 'vue';
import { PANEL_KEY } from '../mount';
import type { Panel } from '../panel';
import IconAction from './IconAction.vue';
import PanelFrame from './PanelFrame.vue';


describe('the panel frame', () => {
  it('has the title, the actions, the status line and the content', () => {
    const wrapper = mount(PanelFrame, {
      props: { title: 'Lighthouse', icon: 'i-lucide-gauge', status: { tone: 'error', text: 'It failed.' } },
      slots: { actions: () => h(IconAction, { icon: 'i-lucide-play', label: 'Run', primary: true }), default: () => h('p', { id: 'body' }, 'content') },
      global: { plugins: [ui] },
    });
    expect(wrapper.find('#title').text()).toBe('Lighthouse');
    const status = wrapper.find('#status');
    expect(status.attributes('role')).toBe('status');
    expect(status.attributes('aria-live')).toBe('assertive');
    expect(status.attributes('data-tone')).toBe('error');
    expect(status.text()).toBe('It failed.');
    expect(wrapper.find('#body').text()).toBe('content');
    const run = wrapper.find('button[aria-label="Run"]');
    expect(run.text()).toBe('Run');
  });

  // RB-094-21: a status line that names a settings path carries the link to it.
  it('shows the settings link of a status line beside its text', async () => {
    const openSettings = vi.fn();
    const wrapper = mount(PanelFrame, {
      props: {
        title: 'Lighthouse',
        icon: 'i-lucide-gauge',
        status: { tone: 'error', text: 'Check Settings > Engines > Node runtime.', setting: { key: 'engines.nodePath', label: 'Open the setting' } },
      },
      global: { plugins: [ui], provide: { [PANEL_KEY as symbol]: { openSettings } as unknown as Panel } },
    });
    const link = wrapper.get('#status [data-testid="setting-link-engines.nodePath"]');
    expect(link.text()).toBe('Open the setting');
    await link.trigger('click');
    expect(openSettings).toHaveBeenCalledWith('engines.nodePath');
  });

  it('shows no status line without a text', () => {
    const wrapper = mount(PanelFrame, { props: { title: 'x', icon: 'i-lucide-gauge', status: { tone: 'info', text: '' } }, global: { plugins: [ui] } });
    expect(wrapper.find('#status').text()).toBe('');
    expect(wrapper.find('#status').attributes('aria-live')).toBe('polite');
  });

  it('gives an icon action its name but no text', () => {
    const wrapper = mount(PanelFrame, {
      props: { title: 'x', icon: 'i-lucide-gauge' },
      slots: { actions: () => h(IconAction, { icon: 'i-lucide-circle-stop', label: 'Cancel' }) },
      global: { plugins: [ui] },
    });
    const button = wrapper.find('button');
    expect(button.attributes('aria-label')).toBe('Cancel');
    expect(button.text()).toBe('');
  });
});
