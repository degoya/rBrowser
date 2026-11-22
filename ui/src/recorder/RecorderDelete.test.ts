// Deleting a recording in the Recorder panel (RB-090-91): "Delete" beside each recording asks in
// the panel first (`automation.recorder.confirmDelete`), sends `recorder.delete`, waits while the
// recording is being recorded or plays, and the core's `recordingsChanged` lists again - the
// recording open in the panel closes when it is gone.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, FlowRunState, RecordingList, ShellToCore } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import type { Recording } from './recording';
import RecorderPage from './RecorderPage.vue';

function recording(name: string): Recording {
  return {
    version: 1,
    name,
    breakpoints: [],
    startedAtMs: 1,
    appVersion: '0.9.0',
    steps: [{ id: 1, action: { type: 'navigate', url: 'http://shop.test/' }, view: 'desktop', atMs: 0 }],
  };
}

const LIST: RecordingList = {
  project: true,
  folder: '.rbrowser/out/recordings',
  recordings: [
    { name: 'login', steps: 1, modifiedMs: 1763380900000 },
    { name: 'old', steps: 1, modifiedMs: 1763380800000 },
  ],
};

interface Options {
  list?: RecordingList;
  run?: FlowRunState;
  /** `automation.recorder.confirmDelete`; absent from the settings when undefined. */
  confirmDelete?: boolean;
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
  let list = options.list ?? LIST;
  const client = new IpcClient({
    send: async (message) => {
      asked.push(message);
      switch (message.type) {
        case 'getRecordings':
          return { type: 'recordings', ...list };
        case 'getRecording':
          return { type: 'recording', name: message.name, recording: recording(message.name) };
        case 'getFlowRun':
          return { type: 'flowRun', ...(options.run ?? { project: true }) };
        case 'getSettings':
          return {
            type: 'settings',
            values:
              options.confirmDelete === undefined
                ? []
                : [{ key: 'automation.recorder.confirmDelete', value: options.confirmDelete, origin: 'project' }],
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
    listNext: (next: RecordingList) => {
      list = next;
    },
  };
}

const commands = (asked: ShellToCore[]) => asked.filter((message) => message.type === 'runCommand');

describe('deleting a recording (RB-090-91)', () => {
  it('asks in the panel, sends recorder.delete and closes the recording once it is gone', async () => {
    const { wrapper, asked, push, listNext } = await mounted();
    await wrapper.get('[data-testid="recording-old"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="recorder-steps"]').exists()).toBe(true);
    const entry = () => wrapper.get('[data-testid="recording-entry-old"]');
    await entry().get('[data-testid="delete"]').trigger('click');
    await settle();
    const question = entry().get('[data-testid="delete-confirm"]');
    expect(question.text()).toContain('Delete the recording "old"?');
    expect(question.text()).toContain('cannot be undone');
    await question.get('[data-testid="delete-no"]').trigger('click');
    await settle();
    expect(commands(asked)).toEqual([]);
    await entry().get('[data-testid="delete"]').trigger('click');
    await settle();
    await entry().get('[data-testid="delete-yes"]').trigger('click');
    await settle();
    expect(commands(asked)).toEqual([{ type: 'runCommand', id: 'recorder.delete', args: { recordingId: 'old' } }]);
    listNext({ ...LIST, recordings: LIST.recordings.slice(0, 1) });
    push({ type: 'recordingsChanged' });
    await settle();
    expect(wrapper.find('[data-testid="recording-old"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="recorder-steps"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="recorder-pick"]').exists()).toBe(true);
  });

  it('keeps the open recording and its edits when the list cannot be read', async () => {
    const { wrapper, push, listNext } = await mounted();
    await wrapper.get('[data-testid="recording-old"]').trigger('click');
    await settle();
    listNext({ ...LIST, recordings: [], error: 'the folder could not be read' });
    push({ type: 'recordingsChanged' });
    await settle();
    expect(wrapper.find('[data-testid="recorder-steps"]').exists()).toBe(true);
  });

  it('deletes at once when automation.recorder.confirmDelete is off', async () => {
    const { wrapper, asked } = await mounted({ confirmDelete: false });
    await wrapper.get('[data-testid="recording-entry-login"] [data-testid="delete"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="delete-confirm"]').exists()).toBe(false);
    expect(commands(asked)).toEqual([{ type: 'runCommand', id: 'recorder.delete', args: { recordingId: 'login' } }]);
  });

  // RB-094-23: a changed setting applies in the open panel, without opening it again.
  it('follows a changed automation.recorder.confirmDelete at once', async () => {
    const options: Options = { confirmDelete: true };
    const { wrapper, asked, push } = await mounted(options);
    options.confirmDelete = false;
    push({ type: 'settingsChanged', keys: ['automation.recorder.confirmDelete'] });
    await settle();
    await wrapper.get('[data-testid="recording-entry-login"] [data-testid="delete"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="delete-confirm"]').exists()).toBe(false);
    expect(commands(asked)).toEqual([{ type: 'runCommand', id: 'recorder.delete', args: { recordingId: 'login' } }]);
  });

  it('keeps Delete waiting while the recording is being recorded or plays, and says why', async () => {
    const recordingNow = await mounted({ list: { ...LIST, recordingNow: 'login' } });
    const login = recordingNow.wrapper.get('[data-testid="recording-entry-login"] [data-testid="delete"]');
    expect(login.attributes('disabled')).toBeDefined();
    expect(login.attributes('title')).toContain('being recorded');
    const old = recordingNow.wrapper.get('[data-testid="recording-entry-old"] [data-testid="delete"]');
    expect(old.attributes('disabled')).toBeUndefined();
    expect(old.attributes('aria-label')).toBe('Delete old');
    recordingNow.wrapper.unmount();
    current = null;
    const playing = await mounted({
      run: { project: true, running: { runId: 'r1', flow: 'old', views: [], startedAtMs: 0 } },
    });
    const played = playing.wrapper.get('[data-testid="recording-entry-old"] [data-testid="delete"]');
    expect(played.attributes('disabled')).toBeDefined();
    expect(played.attributes('title')).toContain('is playing');
  });
});
