import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import type { StateSnapshot } from '../ipc/protocol';
import CellLabelApp from '../cells/CellLabelApp.vue';
import { mountWithShell, settle } from '../test/mountShell';
import DesyncBadge from './DesyncBadge.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const desync = { view: 3, reason: 'notFound', selector: 'nav > a.products', event: 'click' } as const;

describe('the desync badge', () => {
  it('says why the view could not follow and resyncs on click', async () => {
    const { wrapper } = await mountWithShell(DesyncBadge, { mount: { props: { desync, excluded: false } } });
    current = wrapper;
    const badge = wrapper.get('[data-testid="desync-badge"]');
    expect(badge.text()).toBe('desynced');
    // Label in name (RB-090-23, WCAG 2.5.3): the visible text is the name, the reason describes.
    expect(badge.attributes('aria-label')).toBeUndefined();
    expect(badge.attributes('title')).toBe(
      "The click target nav > a.products is missing in this view. Click to load the active view's page.",
    );
    await badge.trigger('click');
    expect(wrapper.emitted('resync')).toEqual([[3]]);
  });

  it('shows an excluded view as not synced, and nothing for a view in sync', async () => {
    const excluded = await mountWithShell(DesyncBadge, { mount: { props: { desync: null, excluded: true } } });
    current = excluded.wrapper;
    expect(excluded.wrapper.get('[data-testid="sync-excluded"]').text()).toBe('not synced');
    excluded.wrapper.unmount();
    const plain = await mountWithShell(DesyncBadge, { mount: { props: { desync: null, excluded: false } } });
    current = plain.wrapper;
    expect(plain.wrapper.find('[data-testid="desync-badge"]').exists()).toBe(false);
    expect(plain.wrapper.find('[data-testid="sync-excluded"]').exists()).toBe(false);
  });

  it('sits in the label strip of its view and sends sync.resync', async () => {
    const state: StateSnapshot = sampleState();
    state.sync = { ...state.sync, desynced: [desync] };
    const { wrapper, mock } = await mountWithShell(CellLabelApp, {
      mock: { state },
      mount: { props: { viewId: 3 } },
    });
    current = wrapper;
    await settle();
    const badge = wrapper.get('[data-testid="cell-badges"] [data-testid="desync-badge"]');
    await badge.trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'sync.resync', args: { view: 3 } });
  });
});
