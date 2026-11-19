// The Recorder panel following a played recording (RB-090-84): the step a view runs is marked,
// a failed step is red with its reason - from the pushed `flowStep`, and from the run's result
// once it ended -, the running step scrolls into sight unless `automation.recorder.followPlayback`
// is off, and each step shows its recorded pause.
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, FlowRunState, FlowStep, JsonValue } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import type { Recording } from './recording';
import RecorderPage from './RecorderPage.vue';

const RUN = '20261118-185842-a1b2c3';

function login(): Recording {
  const click = (id: number, atMs: number) => ({
    id,
    action: { type: 'click' as const, target: { selectors: [{ kind: 'css' as const, path: `#b${id}` }] } },
    view: 'desktop',
    atMs,
  });
  return {
    version: 1,
    name: 'login',
    breakpoints: [],
    startedAtMs: 1,
    appVersion: '0.9.0',
    steps: [
      { id: 1, action: { type: 'navigate', url: 'http://shop.test/' }, view: 'desktop', atMs: 0 },
      click(2, 2_500),
      click(3, 2_600),
    ],
  };
}

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
  vi.restoreAllMocks();
});

async function mounted(settings: Record<string, JsonValue> = {}) {
  const listeners = new Set<(message: CoreToShell) => void>();
  const subscribe = (listener: (message: CoreToShell) => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };
  let run: FlowRunState = { project: true };
  const client = new IpcClient({
    send: async (message) => {
      switch (message.type) {
        case 'getRecordings':
          return {
            type: 'recordings',
            project: true,
            folder: '.rbrowser/out/recordings',
            recordings: [{ name: 'login', steps: 3, modifiedMs: 1 }],
          };
        case 'getRecording':
          // The exporter's pauses (the hover dwell and least pause are its business).
          return {
            type: 'recording',
            name: message.name,
            recording: login(),
            pauses: [
              { step: 1, ms: 0 },
              { step: 2, ms: 2_500 },
              { step: 3, ms: 1_000 },
            ],
          };
        case 'getFlowRun':
          return { type: 'flowRun', ...run };
        case 'getSettings':
          return {
            type: 'settings',
            values: Object.entries(settings).map(([key, value]) => ({ key, value, origin: 'project' as const })),
          };
        default:
          return { type: 'ack' };
      }
    },
    subscribe: () => () => undefined,
  });
  const result = await mountWithShell(RecorderPage, { mount: { props: { client, subscribe } } });
  current = result.wrapper;
  await settle();
  await result.wrapper.get('[data-testid="recording-login"]').trigger('click');
  await settle();
  const push = async (message: CoreToShell) => {
    listeners.forEach((listener) => listener(message));
    await settle();
  };
  return {
    wrapper: result.wrapper,
    push,
    step: (stepId: number, state: FlowStep['state'], message?: string) =>
      push({ type: 'flowStep', runId: RUN, flow: 'login', view: 1, name: `Step ${stepId}: click`, stepId, state, ...(message ? { message } : {}) }),
    end: async (next: FlowRunState) => {
      run = next;
      await push({ type: 'flowRunChanged' });
    },
  };
}

function number(wrapper: VueWrapper, id: number) {
  return wrapper.get(`[data-testid="step-${id}"] [data-testid="step-number"]`);
}

describe('following a played recording (RB-090-84)', () => {
  it('marks the running step, scrolls it into sight and lets go when it passed', async () => {
    const scrolled = vi.fn();
    Element.prototype.scrollIntoView = scrolled;
    const { wrapper, step } = await mounted();
    await step(2, 'running');
    expect(wrapper.get('[data-testid="step-2"]').attributes('aria-current')).toBe('step');
    expect(number(wrapper, 2).attributes('title')).toBe('Playing now');
    expect(scrolled).toHaveBeenCalledTimes(1);
    await step(2, 'passed');
    expect(wrapper.get('[data-testid="step-2"]').attributes('aria-current')).toBeUndefined();
  });

  it('marks a failed step red with its reason', async () => {
    const { wrapper, step } = await mounted();
    await step(3, 'running');
    await step(3, 'failed', 'locator.click: Timeout 10000ms exceeded.');
    expect(wrapper.get('[data-testid="step-3"] [data-testid="step-failure"]').text()).toBe(
      'Failed: locator.click: Timeout 10000ms exceeded.',
    );
    expect(wrapper.find('[data-testid="step-2"] [data-testid="step-failure"]').exists()).toBe(false);
  });

  it('marks the step the result names once the run ended', async () => {
    const { wrapper, step, end } = await mounted();
    await step(2, 'running');
    await end({
      project: true,
      last: {
        version: 1,
        runId: RUN,
        flow: 'login',
        file: 'login.json',
        startedAt: '2026-11-18T18:58:42Z',
        durationMs: 12_000,
        passed: false,
        views: [
          {
            viewId: 1,
            label: 'W1600',
            engine: 'chromium',
            status: 'timedOut',
            durationMs: 12_000,
            steps: [],
            error: {
              message: 'the flow ran longer than 30000 ms',
              step: 'Step 2: click',
              stepId: 2,
              summary: 'the flow ran longer than 30000 ms',
            },
          },
        ],
      },
    });
    expect(wrapper.get('[data-testid="step-2"] [data-testid="step-failure"]').text()).toBe(
      'Failed: the flow ran longer than 30000 ms',
    );
    expect(wrapper.get('[data-testid="step-2"]').attributes('aria-current')).toBeUndefined();
  });

  it('leaves the list where it is with followPlayback off, and ignores other flows', async () => {
    const scrolled = vi.fn();
    Element.prototype.scrollIntoView = scrolled;
    const { wrapper, step, push } = await mounted({ 'automation.recorder.followPlayback': false });
    await step(2, 'running');
    expect(wrapper.get('[data-testid="step-2"]').attributes('aria-current')).toBe('step');
    expect(scrolled).not.toHaveBeenCalled();
    await push({ type: 'flowStep', runId: 'x', flow: 'checkout', view: 1, name: 'Step 3: click', stepId: 3, state: 'running' });
    expect(wrapper.get('[data-testid="step-3"]').attributes('aria-current')).toBeUndefined();
  });

  it("shows the exporter's pause before each step, and none while edits are unsaved", async () => {
    const { wrapper } = await mounted();
    expect(wrapper.find('[data-testid="step-1"] [data-testid="step-pause"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="step-2"] [data-testid="step-pause"]').text()).toBe('+2.5 s');
    expect(wrapper.get('[data-testid="step-3"] [data-testid="step-pause"]').text()).toBe('+1.0 s');
    await wrapper.get('[data-testid="step-2"] [data-testid="step-name"]').setValue('Open the menu');
    await settle();
    expect(wrapper.find('[data-testid="step-pause"]').exists()).toBe(false);
  });
});
