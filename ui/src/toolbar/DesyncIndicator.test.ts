// The desync state in the toolbar (RB-090-23): the native label strips carry no desync badge
// (`cell-label.html` was removed, RB-090-27), so the toolbar shows which views could not
// follow, and a click resyncs them. Its name is its visible label; the views and the reason are
// its description.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { defineComponent, h } from 'vue';
import { sampleState } from '../ipc/mockState';
import type { StateSnapshot } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import DesyncIndicator from './DesyncIndicator.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

function desynced(views: number[]): StateSnapshot {
  const state = sampleState();
  state.sync = {
    ...state.sync,
    desynced: views.map((view) => ({ view, reason: 'notFound', selector: 'nav > a', event: 'click' })),
  };
  return state;
}

describe('the desync indicator', () => {
  it('is not there while every view follows', async () => {
    const { wrapper } = await mountWithShell(DesyncIndicator, { mock: { state: sampleState() } });
    current = wrapper;
    await settle();
    expect(wrapper.find('[data-testid="desync-indicator"]').exists()).toBe(false);
  });

  it('names the desynced views, keeps its label as its name and resyncs them on click', async () => {
    const { wrapper, mock } = await mountWithShell(DesyncIndicator, { mock: { state: desynced([1, 3]) } });
    current = wrapper;
    await settle();
    const button = wrapper.get('[data-testid="desync-indicator"]');
    expect(button.text()).toBe('Desynced (2)');
    // Label in name (WCAG 2.5.3): no aria-label over the visible text.
    expect(button.attributes('aria-label')).toBeUndefined();
    expect(button.attributes('title')).toBe(
      "Mobile, Desktop could not follow the active view. Click to load its page in them.",
    );
    await button.trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'sync.resync' });
  });

  it('hands its focus to the sync switch when the views follow again', async () => {
    const { wrapper, mock } = await mountWithShell(
      defineComponent(() => () =>
        h('div', { 'data-toolbar-group': 'sync' }, [
          h('button', { 'data-testid': 'sync' }, 'Sync'),
          h(DesyncIndicator),
        ]),
      ),
      { mock: { state: desynced([2]) } },
    );
    current = wrapper;
    await settle();
    (wrapper.get('[data-testid="desync-indicator"]').element as HTMLElement).focus();
    const state = desynced([]);
    delete state.sync.desynced;
    mock.state.sync = state.sync;
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    expect(wrapper.find('[data-testid="desync-indicator"]').exists()).toBe(false);
    expect(document.activeElement).toBe(wrapper.get('[data-testid="sync"]').element);
  });
});
