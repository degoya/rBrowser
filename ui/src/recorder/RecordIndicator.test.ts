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

  it('shows the steps beside a pause button and a labelled stop button (RB-090-60)', async () => {
    const state = {
      ...sampleState(),
      recorder: { phase: 'recording' as const, name: 'login', steps: 3 },
    };
    const { wrapper, mock } = await mountWithShell(RecordIndicator, { mock: { state } });
    current = wrapper;
    await settle();
    // The status is text, not a button: the actions are the two buttons beside it.
    const status = wrapper.get('[data-testid="recorder-status"]');
    expect(status.text()).toBe('Recording (3)');
    expect(status.element.tagName).not.toBe('BUTTON');
    const pause = wrapper.get('[data-testid="recorder-pause"]');
    expect(pause.attributes('aria-label')).toBe('Pause recording "login"');
    expect(pause.attributes('title')).toBe('Recording "login": 3 steps so far. Click to pause.');
    await pause.trigger('click');
    const stop = wrapper.get('[data-testid="recorder-stop"]');
    // Label in name (RB-090-23, WCAG 2.5.3): the visible word is the name, the title describes.
    expect(stop.text()).toBe('Stop');
    expect(stop.attributes('aria-label')).toBeUndefined();
    expect(stop.attributes('title')).toBe('Stop recording "login" and save it');
    await stop.trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'recorder.pause' });
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'recorder.stop' });
  });

  it('offers to resume while the recording is paused', async () => {
    const state = {
      ...sampleState(),
      recorder: { phase: 'paused' as const, name: 'login', steps: 3 },
    };
    const { wrapper, mock } = await mountWithShell(RecordIndicator, { mock: { state } });
    current = wrapper;
    await settle();
    expect(wrapper.get('[data-testid="recorder-status"]').text()).toBe('Paused (3)');
    const resume = wrapper.get('[data-testid="recorder-pause"]');
    expect(resume.attributes('aria-label')).toBe('Resume recording "login"');
    expect(resume.attributes('title')).toBe('Recording "login" is paused. Click to resume.');
    await resume.trigger('click');
    await settle();
    expect(mock.sent).toContainEqual({ type: 'runCommand', id: 'recorder.pause' });
  });
});
