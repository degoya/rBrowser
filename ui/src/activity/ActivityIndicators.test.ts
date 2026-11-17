import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import type { StateSnapshot } from '../ipc/protocol';
import RecordIndicator from '../recorder/RecordIndicator.vue';
import DesyncIndicator from '../toolbar/DesyncIndicator.vue';
import { mountWithShell, settle } from '../test/mountShell';
import ActivityIndicators from './ActivityIndicators.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

function withVideo(activityBar: StateSnapshot['activityBar']): StateSnapshot {
  const state = sampleState();
  state.video = { views: [1], startedAtMs: Date.now() - 5_000, maxDurationSec: 600 };
  state.activityBar = activityBar;
  return state;
}

describe('the activities in the toolbar', () => {
  it('show a video that is not in the bar as its time and a stop button', async () => {
    const { wrapper, mock } = await mountWithShell(ActivityIndicators, {
      mock: { state: withVideo(undefined), settings: { 'ui.activityBar.mode': 'indicator' } },
    });
    current = wrapper;
    await settle();
    const indicator = wrapper.get('[data-testid="activity-indicator-video"]');
    expect(indicator.attributes('aria-label')).toBe('Recording video 0:05: Mobile');
    expect(indicator.text()).toContain('0:05');
    const stop = wrapper.get('[data-testid="activity-indicator-video-stop"]');
    expect(stop.attributes('aria-label')).toBe('Stop');
    await stop.trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'capture.video.toggle' });
    // Indicators only: nothing to fold back into a bar.
    expect(wrapper.find('[data-testid="activity-unfold-video"]').exists()).toBe(false);
  });

  it('step back while the entry is in the bar', async () => {
    const { wrapper } = await mountWithShell(ActivityIndicators, { mock: { state: withVideo({ place: 'top' }) } });
    current = wrapper;
    await settle();
    expect(wrapper.find('[data-testid="activity-indicators"]').exists()).toBe(false);
  });

  it('offer to show a folded entry in the bar again', async () => {
    const state = withVideo({ place: 'top', collapsed: ['video'] });
    const { wrapper, mock } = await mountWithShell(ActivityIndicators, { mock: { state } });
    current = wrapper;
    await settle();
    expect(wrapper.find('[data-testid="activity-indicator-video"]').exists()).toBe(true);
    const unfold = wrapper.get('[data-testid="activity-unfold-video"]');
    expect(unfold.attributes('aria-label')).toBe('Show Video in the activity bar');
    await unfold.trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'collapseActivity', activity: 'video', collapsed: false });
    expect(wrapper.find('[data-testid="activity-indicators"]').exists()).toBe(false);
  });

  it('leave the recording and the views out of sync to their indicators, gone while in the bar', async () => {
    const state = sampleState();
    state.recorder = { phase: 'recording', name: 'login', steps: 1 };
    state.sync.desynced = [{ view: 2, reason: 'notFound', event: 'click' }];
    state.activityBar = { place: 'top' };
    const recorder = await mountWithShell(RecordIndicator, { mock: { state } });
    current = recorder.wrapper;
    await settle();
    expect(recorder.wrapper.find('[data-testid="recorder-indicator"]').exists()).toBe(false);
    current.unmount();
    const desync = await mountWithShell(DesyncIndicator, { mock: { state: structuredClone(state) } });
    current = desync.wrapper;
    await settle();
    expect(desync.wrapper.find('[data-testid="desync-indicator"]').exists()).toBe(false);
    current.unmount();
    // Folded, both are back in the toolbar.
    const folded = { ...structuredClone(state), activityBar: { place: 'top' as const, collapsed: ['recorder' as const] } };
    const back = await mountWithShell(RecordIndicator, { mock: { state: folded } });
    current = back.wrapper;
    await settle();
    expect(back.wrapper.find('[data-testid="recorder-indicator"]').exists()).toBe(true);
  });
});
