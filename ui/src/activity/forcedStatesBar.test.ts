// RB-094-16: forced pseudo-class states stand in the activity bar, with Reset and the close
// button like every entry; the bar with the entry passes axe-core and the keyboard checks.

import type { VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import type { StateSnapshot } from '../ipc/protocol';
import { axeViolations } from '../test/axe';
import { unnamedFocusables, unreachableControls } from '../test/keyboard';
import { mountWithShell, settle } from '../test/mountShell';
import ActivityBar from './ActivityBar.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

/** `:hover` forced in views 1 and 2; the core made room at the top. */
function forced(): StateSnapshot {
  const state = sampleState();
  state.forcedStates = [
    { view: 1, states: ['hover'] },
    { view: 2, states: ['hover'] },
  ];
  state.activityBar = { place: 'top' };
  return state;
}

async function mountBar(state: StateSnapshot, settings: Record<string, unknown> = {}) {
  const mounted = await mountWithShell(ActivityBar, { mock: { state, settings }, mount: { props: { place: 'top' } } });
  current = mounted.wrapper;
  await settle();
  return mounted;
}

describe('the forced states entry in the activity bar (RB-094-16)', () => {
  it('names the states and the views, the count in a narrow window', async () => {
    const { wrapper } = await mountBar(forced());
    const entry = wrapper.get('[data-testid="activity-forcedStates"]');
    const text = entry.get('[data-testid="activity-text"]').text();
    expect(text).toContain(':hover');
    expect(text).toContain('Mobile, Tablet');
    expect(entry.attributes('aria-label')).toBe(text);
    expect(entry.get('[data-testid="activity-short"]').text()).toBe('2');
    expect(entry.get('[data-testid="activity-short"]').attributes('title')).toBe(text);
  });

  it('resets every view with the existing command, and folds with its close button', async () => {
    const { wrapper, mock } = await mountBar(forced());
    await wrapper.get('[data-testid="activity-forcedStates-reset"]').trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'view.force_state.reset', args: { target: 'all' } });
    await wrapper.get('[data-testid="activity-forcedStates-close"]').trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'collapseActivity', activity: 'forcedStates', collapsed: true });
  });

  it('is left out with its switch off', async () => {
    const { wrapper } = await mountBar(forced(), { 'ui.activityBar.show.forcedStates': false });
    expect(wrapper.find('[data-testid="activity-forcedStates"]').exists()).toBe(false);
  });

  it('finds no WCAG violation, and every control is reachable and named', async () => {
    await mountBar(forced());
    expect(await axeViolations()).toEqual([]);
    expect(unreachableControls()).toEqual([]);
    expect(unnamedFocusables()).toEqual([]);
  });
});
