// The Recorder panel's playback and its export's save (RB-090-66): "Play" runs `recorder.play`
// for the chosen recording, disabled with the reason while recording, with unsaved edits or while
// a run goes on, which shows "Cancel" (`flows.cancel`); "Save" in the export dialog closes it,
// asks before it replaces a flow, and the panel says where the files went.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, FlowRunState, RecordingExport, RecordingList, ShellToCore } from '../ipc/protocol';
import { IpcError } from '../ipc/transport';
import { mountWithShell, settle } from '../test/mountShell';
import type { Recording } from './recording';
import RecorderPage from './RecorderPage.vue';

function login(): Recording {
  return {
    version: 1,
    name: 'login',
    breakpoints: [],
    startedAtMs: 1,
    appVersion: '0.9.0',
    steps: [{ id: 1, action: { type: 'navigate', url: 'http://shop.test/' }, view: 'desktop', atMs: 0 }],
  };
}

const LIST: RecordingList = {
  project: true,
  folder: '.rbrowser/out/recordings',
  recordings: [{ name: 'login', steps: 1, modifiedMs: 1763380900000 }],
};

const IDLE: FlowRunState = { project: true };
const RUNNING: FlowRunState = {
  project: true,
  running: { runId: '20261117-100000-a1b2c3', flow: 'login', views: [{ view: 1, status: 'running' }], startedAtMs: 0 },
};

const PREVIEW: RecordingExport = {
  written: false,
  files: [{ path: 'login.ts', content: "await page.goto('/');\n" }],
  flowFile: '.rbrowser/flows/login.ts',
};

interface Options {
  list?: RecordingList;
  run?: FlowRunState;
  preview?: RecordingExport;
  refuseSave?: boolean;
  /** `automation.recorder.playbackPace`. */
  pace?: string;
}

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

async function mounted(options: Options = {}) {
  const listeners = new Set<(message: CoreToShell) => void>();
  const subscribe = (listener: (message: CoreToShell) => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };
  const asked: ShellToCore[] = [];
  let run = options.run ?? IDLE;
  const client = new IpcClient({
    send: async (message) => {
      asked.push(message);
      switch (message.type) {
        case 'getRecordings':
          return { type: 'recordings', ...(options.list ?? LIST) };
        case 'getRecording':
          return { type: 'recording', name: message.name, recording: login() };
        case 'getFlowRun':
          return { type: 'flowRun', ...run };
        case 'getSettings':
          return {
            type: 'settings',
            values: [{ key: 'automation.recorder.playbackPace', value: options.pace ?? 'fast', origin: 'default' }],
          };
        case 'exportRecording':
          if (!message.write) {
            return { type: 'recordingExport', ...(options.preview ?? PREVIEW) };
          }
          if (options.refuseSave === true) {
            throw new IpcError(403, 'the flow .rbrowser/flows/login.ts exists already');
          }
          return {
            type: 'recordingExport',
            written: true,
            folder: '.rbrowser/flows',
            files: [{ path: '.rbrowser/flows/login.ts', content: "await page.goto('/');\n" }],
          };
        default:
          return { type: 'ack' };
      }
    },
    subscribe: () => () => undefined,
  });
  const result = await mountWithShell(RecorderPage, {
    mount: { props: { client, subscribe, inlineDialog: true } },
  });
  current = result.wrapper;
  await settle();
  return {
    wrapper: result.wrapper,
    asked,
    push: (message: CoreToShell) => listeners.forEach((listener) => listener(message)),
    runNext: (next: FlowRunState) => {
      run = next;
    },
  };
}

async function pick(wrapper: VueWrapper): Promise<void> {
  await wrapper.get('[data-testid="recording-login"]').trigger('click');
  await settle();
}

describe('playing a recording (RB-090-66)', () => {
  it('plays the chosen recording through recorder.play', async () => {
    const { wrapper, asked } = await mounted();
    await pick(wrapper);
    const play = wrapper.get('[data-testid="recorder-play"]');
    expect(play.attributes('disabled')).toBeUndefined();
    expect(play.text()).toBe('Play');
    await play.trigger('click');
    await settle();
    expect(asked).toContainEqual({
      type: 'runCommand',
      id: 'recorder.play',
      args: { recordingId: 'login', pace: 'fast' },
    });
  });

  it('plays at the pace of the setting unless the person picks another', async () => {
    const { wrapper, asked } = await mounted({ pace: 'recorded' });
    await pick(wrapper);
    await wrapper.get('[data-testid="recorder-play"]').trigger('click');
    await settle();
    expect(asked).toContainEqual({
      type: 'runCommand',
      id: 'recorder.play',
      args: { recordingId: 'login', pace: 'recorded' },
    });
  });

  it('cannot play while recording or with unsaved edits, and says why', async () => {
    const recording = await mounted({ list: { ...LIST, recordingNow: 'checkout' } });
    await pick(recording.wrapper);
    const busy = recording.wrapper.get('[data-testid="recorder-play"]');
    expect(busy.attributes('disabled')).toBeDefined();
    expect(busy.attributes('title')).toBe('Stop the recording before playing one.');
    recording.wrapper.unmount();
    current = null;

    const { wrapper } = await mounted();
    await pick(wrapper);
    await wrapper.get('[data-testid="step-1"] [data-testid="step-name"]').setValue('Open the shop');
    await settle();
    const unsaved = wrapper.get('[data-testid="recorder-play"]');
    expect(unsaved.attributes('disabled')).toBeDefined();
    expect(unsaved.attributes('title')).toBe('Save or revert the changes before playing.');
  });

  it('shows the run going on with Cancel, and Play again once it ended', async () => {
    const { wrapper, asked, push, runNext } = await mounted({ run: RUNNING });
    await pick(wrapper);
    expect(wrapper.get('[data-testid="recorder-running"]').text()).toBe('login is running');
    expect(wrapper.get('[data-testid="recorder-play"]').attributes('disabled')).toBeDefined();
    await wrapper.get('[data-testid="recorder-cancel"]').trigger('click');
    await settle();
    expect(asked).toContainEqual({ type: 'runCommand', id: 'flows.cancel' });
    runNext(IDLE);
    push({ type: 'flowRunChanged' });
    await settle();
    expect(wrapper.find('[data-testid="recorder-cancel"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="recorder-play"]').attributes('disabled')).toBeUndefined();
  });
});

describe('saving an export (RB-090-66)', () => {
  it('names where the flow goes, closes on save and says where it went', async () => {
    const { wrapper, asked } = await mounted();
    await pick(wrapper);
    await wrapper.get('[data-testid="recorder-export"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="export-flow-file"]').text()).toContain('.rbrowser/flows/login.ts');
    expect(wrapper.find('[data-testid="export-overwrite"]').exists()).toBe(false);
    await wrapper.get('[data-testid="export-save"]').trigger('click');
    await settle();
    expect(asked).toContainEqual({ type: 'exportRecording', name: 'login', format: 'flow', exportName: 'login', write: true });
    expect(wrapper.find('[data-testid="export-save"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="recorder-notice"]').text()).toBe('Saved in .rbrowser/flows.');
  });

  it('replaces an existing flow only when the person ticks it', async () => {
    const { wrapper, asked } = await mounted({ preview: { ...PREVIEW, flowExists: true } });
    await pick(wrapper);
    await wrapper.get('[data-testid="recorder-export"]').trigger('click');
    await settle();
    const save = wrapper.get('[data-testid="export-save"]');
    expect(save.attributes('disabled')).toBeDefined();
    await wrapper.get('[data-testid="export-overwrite"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="export-save"]').attributes('disabled')).toBeUndefined();
    await wrapper.get('[data-testid="export-save"]').trigger('click');
    await settle();
    expect(asked).toContainEqual({
      type: 'exportRecording',
      name: 'login',
      format: 'flow',
      exportName: 'login',
      write: true,
      overwrite: true,
    });
  });

  it('stays open and says why when the core refuses the save', async () => {
    const { wrapper } = await mounted({ refuseSave: true });
    await pick(wrapper);
    await wrapper.get('[data-testid="recorder-export"]').trigger('click');
    await settle();
    await wrapper.get('[data-testid="export-save"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="export-save"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="export-error"]').text()).toContain('exists already');
  });
});
