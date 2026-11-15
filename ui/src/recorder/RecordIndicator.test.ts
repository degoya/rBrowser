import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { sampleState } from '../ipc/mockState';
import { mountWithShell, settle } from '../test/mountShell';
import RecordIndicator from './RecordIndicator.vue';
import { recorderIndicator } from './indicator';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

describe('the recording indicator', () => {
  it('follows the snapshot: nothing while idle, the phase, name and steps while recording', () => {
    const state = sampleState();
    expect(recorderIndicator(null)).toBeNull();
    expect(recorderIndicator(state)).toBeNull();
    state.recorder = { phase: 'paused', name: 'login', steps: 4 };
    expect(recorderIndicator(state)).toEqual({ paused: true, name: 'login', steps: 4 });
  });

  it('is hidden while no recording runs', async () => {
    const { wrapper } = await mountWithShell(RecordIndicator);
    current = wrapper;
    await settle();
    expect(wrapper.find('[data-testid="recorder-indicator"]').exists()).toBe(false);
  });

  it('shows the steps, pauses on the first button and stops on the second', async () => {
    const state = {
      ...sampleState(),
      recorder: { phase: 'recording' as const, name: 'login', steps: 3 },
    };
    const { wrapper, mock } = await mountWithShell(RecordIndicator, { mock: { state } });
    current = wrapper;
    await settle();
    const pause = wrapper.get('[data-testid="recorder-pause"]');
    expect(pause.text()).toBe('Recording (3)');
    // Label in name (RB-090-23, WCAG 2.5.3): the visible text is the name, the title describes.
    expect(pause.attributes('aria-label')).toBeUndefined();
    expect(pause.attributes('title')).toBe('Recording "login": 3 steps so far. Click to pause.');
    await pause.trigger('click');
    const stop = wrapper.get('[data-testid="recorder-stop"]');
    expect(stop.attributes('aria-label')).toBe('Stop recording "login" and save it');
    await stop.trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'recorder.pause' });
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'recorder.stop' });
  });

  it('says when the recording is paused', async () => {
    const state = {
      ...sampleState(),
      recorder: { phase: 'paused' as const, name: 'login', steps: 3 },
    };
    const { wrapper } = await mountWithShell(RecordIndicator, { mock: { state } });
    current = wrapper;
    await settle();
    const pause = wrapper.get('[data-testid="recorder-pause"]');
    expect(pause.text()).toBe('Paused (3)');
    expect(pause.attributes('title')).toBe('Recording "login" is paused. Click to resume.');
  });
});
