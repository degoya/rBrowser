// The Recorder panel never replaces unsaved edits under the person's hands (RB-092-01 review):
// another recording chosen with a dirty draft asks first, and the chosen recording changed on
// disk (an agent's step tools, a continued recording) is read again when nothing is unsaved, or
// marked "changed on disk" - Save waiting, Revert reading the file - when something is.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, RecordingList, ShellToCore } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import type { Recording } from './recording';
import RecorderPage from './RecorderPage.vue';

function recording(name: string, url = 'https://example.test/'): Recording {
  return {
    version: 1,
    name,
    breakpoints: [],
    startedAtMs: 1,
    appVersion: '0.9.2',
    steps: [{ id: 1, action: { type: 'navigate', url }, view: 'desktop', atMs: 0 }],
  };
}

function list(modifiedMs: number): RecordingList {
  return {
    project: true,
    folder: '.rbrowser/out/recordings',
    recordings: [
      { name: 'search', steps: 1, modifiedMs },
      { name: 'cart', steps: 1, modifiedMs: 1000 },
    ],
  };
}

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

async function opened() {
  const listeners = new Set<(message: CoreToShell) => void>();
  const asked: ShellToCore[] = [];
  const disk = { modifiedMs: 2000, url: 'https://example.test/' };
  const client = new IpcClient({
    send: async (message) => {
      asked.push(message);
      switch (message.type) {
        case 'getRecordings':
        case 'saveRecording':
          return { type: 'recordings', ...list(disk.modifiedMs) };
        case 'getRecording':
          return { type: 'recording', name: message.name, recording: recording(message.name, disk.url) };
        default:
          return { type: 'ack' };
      }
    },
    subscribe: () => () => undefined,
  });
  const subscribe = (listener: (message: CoreToShell) => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };
  const { wrapper } = await mountWithShell(RecorderPage, { mount: { props: { client, subscribe, inlineDialog: true } } });
  current = wrapper;
  await settle();
  await wrapper.get('[data-testid="recording-search"]').trigger('click');
  await settle();
  /** An agent changed the file: the core says the recordings changed. */
  const changeOnDisk = async (url: string) => {
    disk.modifiedMs += 1000;
    disk.url = url;
    listeners.forEach((listener) => listener({ type: 'recordingsChanged' }));
    await settle();
  };
  return { wrapper, asked, changeOnDisk };
}

const opens = (asked: ShellToCore[], name: string) =>
  asked.filter((message) => message.type === 'getRecording' && message.name === name).length;

async function makeDirty(wrapper: VueWrapper): Promise<void> {
  await wrapper.get('[data-testid="step-1"] [data-testid="step-name"]').setValue('Open the start');
  await settle();
  expect(wrapper.find('[data-testid="recorder-dirty"]').exists()).toBe(true);
}

describe('unsaved edits in the Recorder panel', () => {
  it('ask before another recording replaces them, and stay when kept', async () => {
    const { wrapper, asked } = await opened();
    await makeDirty(wrapper);
    await wrapper.get('[data-testid="recording-cart"]').trigger('click');
    await settle();
    expect(opens(asked, 'cart')).toBe(0);
    expect(wrapper.find('[data-testid="recorder-discard"]').exists()).toBe(true);
    await wrapper.get('[data-testid="recorder-discard-keep"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="recorder-discard"]').exists()).toBe(false);
    expect((wrapper.get('[data-testid="step-1"] [data-testid="step-name"]').element as HTMLInputElement).value).toBe('Open the start');
  });

  it('go once the person says so, and the other recording opens', async () => {
    const { wrapper, asked } = await opened();
    await makeDirty(wrapper);
    await wrapper.get('[data-testid="recording-cart"]').trigger('click');
    await settle();
    await wrapper.get('[data-testid="recorder-discard-confirm"]').trigger('click');
    await settle();
    expect(opens(asked, 'cart')).toBe(1);
    expect(wrapper.find('[data-testid="recorder-dirty"]').exists()).toBe(false);
  });

  it('a recording changed on disk is read again while nothing is unsaved', async () => {
    const { wrapper, asked, changeOnDisk } = await opened();
    expect(opens(asked, 'search')).toBe(1);
    await changeOnDisk('https://example.test/new');
    expect(opens(asked, 'search')).toBe(2);
    expect(wrapper.find('[data-testid="recorder-stale"]').exists()).toBe(false);
  });

  it('a recording changed on disk under unsaved edits blocks Save until Revert reads it', async () => {
    const { wrapper, asked, changeOnDisk } = await opened();
    await makeDirty(wrapper);
    await changeOnDisk('https://example.test/new');
    expect(opens(asked, 'search')).toBe(1);
    expect(wrapper.find('[data-testid="recorder-stale"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="recorder-save"]').attributes('disabled')).toBeDefined();
    await wrapper.get('[data-testid="recorder-stale-revert"]').trigger('click');
    await settle();
    expect(opens(asked, 'search')).toBe(2);
    expect(wrapper.find('[data-testid="recorder-stale"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="recorder-dirty"]').exists()).toBe(false);
  });

  it('its own save is no change on disk', async () => {
    const { wrapper, asked } = await opened();
    await makeDirty(wrapper);
    await wrapper.get('[data-testid="recorder-save"]').trigger('click');
    await settle();
    expect(asked.some((message) => message.type === 'saveRecording')).toBe(true);
    expect(wrapper.find('[data-testid="recorder-stale"]').exists()).toBe(false);
  });
});
