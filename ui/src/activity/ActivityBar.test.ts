import { afterEach, describe, expect, it } from 'vitest';
import { defineComponent, h } from 'vue';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import type { StateSnapshot } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import ActivityBar from './ActivityBar.vue';
import { useActivities } from './store';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

/** A video of two views, a recording and a view out of sync; the core made room at the top. */
function busy(): StateSnapshot {
  const state = sampleState();
  state.video = { views: [1, 2], startedAtMs: Date.now() - 61_000, maxDurationSec: 600 };
  state.recorder = { phase: 'recording', name: 'login', steps: 3 };
  state.sync.desynced = [{ view: 2, reason: 'notFound', selector: 'nav', event: 'click' }];
  state.activityBar = { place: 'top' };
  return state;
}

async function mountBar(place: 'top' | 'bottom', state: StateSnapshot, settings: Record<string, unknown> = {}) {
  const mounted = await mountWithShell(ActivityBar, { mock: { state, settings }, mount: { props: { place } } });
  current = mounted.wrapper;
  await settle();
  return mounted;
}

describe('the activity bar', () => {
  it('shows one entry per running activity, side by side, in the bar order', async () => {
    const { wrapper } = await mountBar('top', busy());
    const bar = wrapper.get('[data-testid="activity-bar"]');
    expect(bar.attributes('role')).toBe('toolbar');
    expect(bar.attributes('data-place')).toBe('top');
    const groups = bar.findAll('[role="group"]').map((group) => group.attributes('data-testid'));
    expect(groups).toEqual(['activity-video', 'activity-recorder', 'activity-desync']);
    const video = wrapper.get('[data-testid="activity-video"]');
    expect(video.get('[data-testid="activity-text"]').text()).toBe('Recording video 1:01: Mobile, Tablet');
    // The narrow window's form: the counter, the state as its tooltip.
    expect(video.get('[data-testid="activity-short"]').text()).toBe('1:01');
    expect(wrapper.get('[data-testid="activity-recorder"]').attributes('aria-label')).toBe(
      'Recording "login": 3 steps',
    );
  });

  it('runs the registry commands of its controls', async () => {
    const { wrapper, mock } = await mountBar('top', busy());
    await wrapper.get('[data-testid="activity-video-stop"]').trigger('click');
    await wrapper.get('[data-testid="activity-recorder-pause"]').trigger('click');
    await wrapper.get('[data-testid="activity-recorder-check"]').trigger('click');
    await wrapper.get('[data-testid="activity-recorder-stop"]').trigger('click');
    await wrapper.get('[data-testid="activity-desync-resync"]').trigger('click');
    await settle();
    for (const id of ['capture.video.toggle', 'recorder.pause', 'recorder.check', 'recorder.stop', 'sync.resync']) {
      expect(mock.sent).toContainEqual({ type: 'runCommand', id });
    }
    // The primary control has its word, the others are icons with a name.
    expect(wrapper.get('[data-testid="activity-recorder-stop"]').text()).toBe('Stop');
    expect(wrapper.get('[data-testid="activity-recorder-pause"]').attributes('aria-label')).toBe('Pause recording');
  });

  it('folds an entry into its toolbar indicator with its close button', async () => {
    const { wrapper, mock } = await mountBar('top', busy());
    const close = wrapper.get('[data-testid="activity-video-close"]');
    expect(close.attributes('aria-label')).toBe('Move Video to the toolbar');
    await close.trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'collapseActivity', activity: 'video', collapsed: true });
    expect(wrapper.find('[data-testid="activity-video"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="activity-recorder"]').exists()).toBe(true);
  });

  it('offers the recording just stopped for Play last, until played', async () => {
    const state = sampleState();
    state.activityBar = { place: 'top', recorded: 'login' };
    const { wrapper, mock } = await mountBar('top', state);
    const entry = wrapper.get('[data-testid="activity-recorder"]');
    expect(entry.get('[data-testid="activity-text"]').text()).toBe('Recorded "login"');
    await wrapper.get('[data-testid="activity-recorder-play"]').trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'recorder.play' });
    expect(mock.sent).toContainEqual({ type: 'collapseActivity', activity: 'recorder', collapsed: true });
    expect(wrapper.find('[data-testid="activity-bar"]').exists()).toBe(false);
  });

  it('is drawn only where the core made room, and not at all with indicators only', async () => {
    expect((await mountBar('bottom', busy())).wrapper.find('[data-testid="activity-bar"]').exists()).toBe(false);
    current?.unmount();
    const unplaced = { ...busy(), activityBar: {} };
    expect((await mountBar('top', unplaced)).wrapper.find('[data-testid="activity-bar"]').exists()).toBe(false);
    current?.unmount();
    const indicator = await mountBar('top', busy(), { 'ui.activityBar.mode': 'indicator' });
    expect(indicator.wrapper.find('[data-testid="activity-bar"]').exists()).toBe(false);
  });

  it('leaves out an activity switched off in the settings', async () => {
    const { wrapper } = await mountBar('top', busy(), { 'ui.activityBar.show.desync': false });
    expect(wrapper.find('[data-testid="activity-desync"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="activity-video"]').exists()).toBe(true);
  });
});

describe('the room for the bar', () => {
  const Room = defineComponent({
    setup() {
      useActivities().manageRoom();
      return () => h('div');
    },
  });

  it('is asked for while something runs and given back after the last activity', async () => {
    const state = sampleState();
    state.video = { views: [1], startedAtMs: Date.now(), maxDurationSec: 600 };
    const { wrapper, mock } = await mountWithShell(Room, { mock: { state } });
    current = wrapper;
    await settle();
    expect(mock.sent).toContainEqual({ type: 'activityBar', place: 'top' });
    expect(mock.state.activityBar?.place).toBe('top');
    delete mock.state.video;
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'activityBar' });
  });

  it('is asked for at the bottom when the settings say so, and never with indicators only', async () => {
    const state = sampleState();
    state.sync.desynced = [{ view: 2, reason: 'notFound', event: 'click' }];
    const bottom = await mountWithShell(Room, { mock: { state, settings: { 'ui.activityBar.position': 'bottom' } } });
    current = bottom.wrapper;
    await settle();
    expect(bottom.mock.sent).toContainEqual({ type: 'activityBar', place: 'bottom' });
    current.unmount();
    const indicator = await mountWithShell(Room, {
      mock: { state: structuredClone(state), settings: { 'ui.activityBar.mode': 'indicator' } },
    });
    current = indicator.wrapper;
    await settle();
    expect(indicator.mock.sent.filter((message) => message.type === 'activityBar')).toEqual([{ type: 'activityBar' }]);
  });

  it('opens a folded entry again once its activity ended', async () => {
    const state = sampleState();
    state.activityBar = { collapsed: ['video'] };
    const { wrapper, mock } = await mountWithShell(Room, { mock: { state } });
    current = wrapper;
    await settle();
    expect(mock.sent).toContainEqual({ type: 'collapseActivity', activity: 'video', collapsed: false });
  });
});
