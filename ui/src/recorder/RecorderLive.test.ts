// The Recorder panel while a recording runs (RB-090-89): each step shows as soon as the core
// pushes it (`recordingSteps`), read-only, the newest marked and scrolled into sight, a hover with
// its pause and pointer path; the recording's end opens the saved recording, the setting
// switched off only hides the list, and a panel opened during a recording catches up.
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, ShellToCore } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import type { Recording, Step } from './recording';
import RecorderPage from './RecorderPage.vue';

const NAVIGATE: Step = { id: 1, action: { type: 'navigate', url: 'http://localhost:3000/' }, view: 'w2000', atMs: 0 };
const HOVER: Step = {
  id: 2,
  action: { type: 'hover', target: { selectors: [{ kind: 'css', path: '#main-nav > a' }] } },
  view: 'w2000',
  atMs: 1_800,
  path: [
    { x: 1998, y: 27, t: 0 },
    { x: 1907, y: 31, t: 23 },
  ],
};

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
  vi.restoreAllMocks();
});

async function mounted() {
  const listeners = new Set<(message: CoreToShell) => void>();
  const subscribe = (listener: (message: CoreToShell) => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };
  const asked: ShellToCore['type'][] = [];
  let saved = false;
  const recording: Recording = {
    version: 1,
    name: 'rec',
    breakpoints: [],
    startedAtMs: 1,
    appVersion: '0.9.0',
    steps: [NAVIGATE, HOVER],
  };
  const client = new IpcClient({
    send: async (message) => {
      asked.push(message.type);
      switch (message.type) {
        case 'getRecordings':
          return {
            type: 'recordings',
            project: true,
            folder: '.rbrowser/out/recordings',
            recordings: saved ? [{ name: 'rec', steps: 2, modifiedMs: 2 }] : [],
          };
        case 'getRecording':
          return { type: 'recording', name: message.name, recording };
        case 'getFlowRun':
          return { type: 'flowRun', project: true };
        case 'getSettings':
          return { type: 'settings', values: [] };
        default:
          return { type: 'ack' };
      }
    },
    subscribe: () => () => undefined,
  });
  const result = await mountWithShell(RecorderPage, { mount: { props: { client, subscribe } } });
  current = result.wrapper;
  await settle();
  return {
    wrapper: result.wrapper,
    asked,
    save: () => {
      saved = true;
    },
    push: async (message: CoreToShell) => {
      listeners.forEach((listener) => listener(message));
      await settle();
    },
  };
}

describe('the steps while recording (RB-090-89)', () => {
  it('asks for the steps so far when it opens', async () => {
    const { asked } = await mounted();
    expect(asked).toContain('resendRecordingSteps');
  });

  it('only lets the list go when the setting is switched off', async () => {
    const { wrapper, push, asked, save } = await mounted();
    await push({ type: 'recordingSteps', name: 'rec', state: 'recording', from: 0, steps: [NAVIGATE] });
    save();
    asked.length = 0;
    await push({ type: 'recordingSteps', name: 'rec', state: 'hidden', from: 0, steps: [] });
    expect(wrapper.find('[data-testid="recorder-live"]').exists()).toBe(false);
    expect(asked).not.toContain('getRecording');
    expect(wrapper.find('[data-testid="recorder-pick"]').exists()).toBe(true);
  });

  it('lists each step as it comes, read-only, the newest marked and in sight', async () => {
    const scrolled = vi.fn();
    Element.prototype.scrollIntoView = scrolled;
    const { wrapper, push } = await mounted();
    await push({ type: 'recordingSteps', name: 'rec', state: 'recording', from: 0, steps: [] });
    expect(wrapper.get('[data-testid="recorder-live"]').text()).toContain('rec');
    expect(wrapper.find('[data-testid="recorder-live-empty"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="recorder-start"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="recording-now"]').exists()).toBe(true);

    await push({ type: 'recordingSteps', name: 'rec', state: 'recording', from: 0, steps: [NAVIGATE] });
    await push({ type: 'recordingSteps', name: 'rec', state: 'recording', from: 1, steps: [HOVER] });
    const rows = wrapper.findAll('[data-testid="recorder-live-steps"] > li');
    expect(rows).toHaveLength(2);
    expect(wrapper.get('[data-testid="step-2"]').attributes('data-newest')).toBe('true');
    expect(wrapper.get('[data-testid="step-1"]').attributes('data-newest')).toBeUndefined();
    expect(scrolled).toHaveBeenCalled();
    // The hover with the pause before it and its pointer path, as the saved view shows them.
    expect(wrapper.get('[data-testid="step-2"] [data-testid="step-pause"]').text()).toBe('+1.8 s');
    expect(wrapper.get('[data-testid="step-2"] [data-testid="step-path"]').text()).toBe('Pointer path (2 points)');
    // Read-only: no buttons, no fields.
    expect(wrapper.find('[data-testid="recorder-live"] [data-testid="step-remove"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="recorder-live"] [data-testid="step-name"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="recorder-live-count"]').text()).toBe('2 steps');
  });

  it('replaces a cleaned-up step in place', async () => {
    const { wrapper, push } = await mounted();
    await push({ type: 'recordingSteps', name: 'rec', state: 'recording', from: 0, steps: [NAVIGATE, HOVER] });
    const moved = { ...HOVER, path: [...(HOVER.path ?? []), { x: 1800, y: 40, t: 60 }] };
    await push({ type: 'recordingSteps', name: 'rec', state: 'recording', from: 1, steps: [moved] });
    expect(wrapper.findAll('[data-testid="recorder-live-steps"] > li')).toHaveLength(2);
    expect(wrapper.get('[data-testid="step-2"] [data-testid="step-path"]').text()).toBe('Pointer path (3 points)');
  });

  it('opens the saved recording when the recording ended', async () => {
    const { wrapper, push, asked, save } = await mounted();
    await push({ type: 'recordingSteps', name: 'rec', state: 'recording', from: 0, steps: [NAVIGATE] });
    save();
    asked.length = 0;
    await push({ type: 'recordingSteps', name: 'rec', state: 'ended', from: 0, steps: [] });
    expect(wrapper.find('[data-testid="recorder-live"]').exists()).toBe(false);
    expect(asked).toContain('getRecordings');
    expect(asked).toContain('getRecording');
    // The saved recording, editable again.
    expect(wrapper.find('[data-testid="step-2"] [data-testid="step-name"]').exists()).toBe(true);
  });
});
