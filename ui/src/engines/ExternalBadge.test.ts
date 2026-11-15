import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import type { ExternalView, StateSnapshot } from '../ipc/protocol';
import CellLabelApp from '../cells/CellLabelApp.vue';
import { mountWithShell, settle } from '../test/mountShell';
import ExternalBadge from './ExternalBadge.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

function external(status: ExternalView['status'], extra: Partial<ExternalView> = {}): ExternalView {
  return { view: 3, label: 'Phone', breakpoint: 'phone', engine: 'firefox', status, url: '', ...extra };
}

async function badge(view: ExternalView | null): Promise<VueWrapper> {
  const { wrapper } = await mountWithShell(ExternalBadge, { mount: { props: { external: view } } });
  current = wrapper;
  return wrapper;
}

describe('the external view badge', () => {
  it('says the engine is not available yet, with the reason', async () => {
    const wrapper = await badge(external('unavailable', { reason: 'Firefox is not available yet' }));
    const status = wrapper.get('[data-testid="external-status"]');
    expect(status.text()).toBe('Firefox: not available yet');
    expect(status.attributes('title')).toBe(
      'Firefox is not available yet. Chromium shows this view until then.',
    );
    expect(wrapper.find('[data-testid="external-restart"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="external-show"]').exists()).toBe(false);
  });

  it('offers a restart once the window ended, by keyboard too', async () => {
    const wrapper = await badge(external('exited'));
    expect(wrapper.get('[data-testid="external-status"]').text()).toBe('Firefox: engine stopped');
    const restart = wrapper.get('[data-testid="external-restart"]');
    expect(restart.element.tagName).toBe('BUTTON');
    expect(restart.text()).toBe('Restart');
    await restart.trigger('click');
    expect(wrapper.emitted('restart')).toEqual([[3]]);
  });

  it('offers to open a window the person closed again (RB-080-38)', async () => {
    const wrapper = await badge(external('closed'));
    const status = wrapper.get('[data-testid="external-status"]');
    expect(status.text()).toBe('Firefox: window closed');
    expect(status.attributes('title')).toBe(
      'The Firefox window was closed. Open again opens it with its last page.',
    );
    const reopen = wrapper.get('[data-testid="external-restart"]');
    expect(reopen.text()).toBe('Open again');
    await reopen.trigger('click');
    expect(wrapper.emitted('restart')).toEqual([[3]]);
  });

  it('shows the window of a running view and lists what it does not emulate', async () => {
    const wrapper = await badge(external('running', { engine: 'webkit', unsupported: ['platform', 'textScale'] }));
    const status = wrapper.get('[data-testid="external-status"]');
    expect(status.text()).toBe('WebKit (not Safari): external window');
    expect(status.attributes('title')).toBe('Not emulated in WebKit (not Safari): platform, textScale');
    await wrapper.get('[data-testid="external-show"]').trigger('click');
    expect(wrapper.emitted('show')).toEqual([[3]]);
  });

  it('says the view sits in its cell and offers no window to show (RB-080-39)', async () => {
    const wrapper = await badge(external('running', { embedded: true }));
    const status = wrapper.get('[data-testid="external-status"]');
    // The catalogue has the texts once the wave's locale parts are merged; before, the keys show.
    expect(status.text()).toMatch(/^Firefox: (in its cell|engines\.external\.status\.embedded)$/);
    expect(status.attributes('title')).toMatch(
      /^(This view runs in its Firefox window inside its cell\.|engines\.external\.hint\.embedded)/,
    );
    expect(wrapper.find('[data-testid="external-show"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="external-restart"]').exists()).toBe(false);
    // A window of its own again (released, or the embedding failed): "Show window" is back.
    const released = await badge(external('running', { embedded: false }));
    expect(released.get('[data-testid="external-status"]').text()).toBe('Firefox: external window');
    expect(released.find('[data-testid="external-show"]').exists()).toBe(true);
  });

  it('shows nothing for a grid view', async () => {
    const wrapper = await badge(null);
    expect(wrapper.find('[data-testid="external-badge"]').exists()).toBe(false);
  });

  it('sits in the label strip of its cell and runs the restart command', async () => {
    const state: StateSnapshot = { ...sampleState(), externals: [external('failed', { view: 1, reason: 'no Firefox found' })] };
    const { wrapper, mock } = await mountWithShell(CellLabelApp, {
      mock: { state },
      mount: { props: { viewId: 1 } },
    });
    current = wrapper;
    await settle();
    expect(wrapper.get('[data-testid="external-status"]').attributes('title')).toBe(
      'The Firefox window could not be started: no Firefox found',
    );
    await wrapper.get('[data-testid="external-restart"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'engine.restart', args: { view: 1 } });
  });
});
