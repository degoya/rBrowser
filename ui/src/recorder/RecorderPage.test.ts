import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, RecordingExport, RecordingList, ShellToCore } from '../ipc/protocol';
import { IpcError } from '../ipc/transport';
import { mountWithShell, settle } from '../test/mountShell';
import type { Recording } from './recording';
import RecorderPage from './RecorderPage.vue';

const css = (path: string) => ({ selectors: [{ kind: 'css' as const, path }] });

function login(): Recording {
  return {
    version: 1,
    name: 'login',
    breakpoints: [],
    startedAtMs: 1,
    appVersion: '0.9.0',
    steps: [
      { id: 1, action: { type: 'navigate', url: 'http://shop.test/' }, view: 'desktop', atMs: 0 },
      { id: 2, action: { type: 'click', target: css('#burger') }, view: 'mobile', atMs: 300, onlyIn: ['mobile'] },
      { id: 3, action: { type: 'fill', target: css('#email'), value: 'ada@shop.test' }, view: 'desktop', atMs: 900 },
    ],
  };
}

const LIST: RecordingList = {
  project: true,
  folder: '.rbrowser/out/recordings',
  recordings: [
    { name: 'login', steps: 3, modifiedMs: 1763380900000 },
    { name: 'cart', steps: 5, modifiedMs: 1763380000000 },
  ],
};

const EXPORT: RecordingExport = {
  written: false,
  files: [{ path: 'login.ts', content: "// Step 1: navigate\nawait page.goto('/');\n" }],
};

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

async function mounted(list: RecordingList = LIST, saveFails = false) {
  const listeners = new Set<(message: CoreToShell) => void>();
  const subscribe = (listener: (message: CoreToShell) => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };
  const asked: ShellToCore[] = [];
  let answer = list;
  const client = new IpcClient({
    send: async (message) => {
      asked.push(message);
      switch (message.type) {
        case 'getRecordings':
          return { type: 'recordings', ...answer };
        case 'getRecording':
          return { type: 'recording', name: message.name, recording: login() };
        case 'saveRecording':
          if (saveFails) {
            throw new IpcError(400, 'two steps have the same id');
          }
          return { type: 'recordings', ...answer };
        case 'exportRecording':
          return message.write
            ? { type: 'recordingExport', written: true, folder: '.rbrowser/out/exports/login', files: EXPORT.files }
            : { type: 'recordingExport', ...EXPORT };
        default:
          return { type: 'ack' };
      }
    },
    subscribe: () => () => undefined,
  });
  const copied: string[] = [];
  const clipboard = { writeText: async (text: string) => void copied.push(text) };
  const result = await mountWithShell(RecorderPage, {
    mount: { props: { client, subscribe, clipboard, inlineDialog: true } },
  });
  current = result.wrapper;
  await settle();
  return {
    wrapper: result.wrapper,
    asked,
    copied,
    push: (message: CoreToShell) => listeners.forEach((listener) => listener(message)),
    answerNext: (next: RecordingList) => {
      answer = next;
    },
  };
}

async function pick(wrapper: VueWrapper, name: string): Promise<void> {
  await wrapper.get(`[data-testid="recording-${name}"]`).trigger('click');
  await settle();
}

describe('the Recorder panel (RB-090-60)', () => {
  it('lists the recordings newest first and offers to start one', async () => {
    const { wrapper, asked } = await mounted();
    expect(wrapper.find('[data-testid="recording-login"]').text()).toContain('login');
    expect(wrapper.find('[data-testid="recording-login"]').text()).toContain('3 steps');
    expect(wrapper.find('[data-testid="recorder-pick"]').exists()).toBe(true);
    await wrapper.get('[data-testid="recorder-start"]').trigger('click');
    await settle();
    expect(asked).toContainEqual({ type: 'runCommand', id: 'recorder.start' });
  });

  it('shows the running recording instead of the start button', async () => {
    const { wrapper } = await mounted({ ...LIST, recordingNow: 'checkout' });
    expect(wrapper.get('[data-testid="recording-now"]').text()).toBe('Recording "checkout"');
    expect(wrapper.find('[data-testid="recorder-start"]').exists()).toBe(false);
  });

  it('stops the running recording where it was started (RB-090-94)', async () => {
    const { wrapper, asked } = await mounted({ ...LIST, recordingNow: 'checkout' });
    const stop = wrapper.get('[data-testid="recorder-stop"]');
    expect(stop.text()).toBe('Stop recording');
    await stop.trigger('click');
    await settle();
    expect(asked).toContainEqual({ type: 'runCommand', id: 'recorder.stop' });
  });

  it('shows a recording\'s steps with kind, element, value and onlyIn', async () => {
    const { wrapper } = await mounted();
    await pick(wrapper, 'login');
    expect(wrapper.findAll('li[data-testid^="step-"]')).toHaveLength(3);
    const click = wrapper.get('[data-testid="step-2"]');
    expect(click.get('[data-testid="step-kind"]').text()).toBe('Click');
    expect(click.get('[data-testid="step-target"]').text()).toBe('#burger');
    expect(click.get('[data-testid="step-only-in"]').text()).toContain('mobile');
    expect(click.find('[data-testid="step-value"]').exists()).toBe(false);
    const fill = wrapper.get('[data-testid="step-3"]');
    expect((fill.get('[data-testid="step-value"]').element as HTMLInputElement).value).toBe('ada@shop.test');
    // The first step cannot move up, the last not down.
    expect(wrapper.get('[data-testid="step-1"] [data-testid="step-up"]').attributes('disabled')).toBeDefined();
    expect(wrapper.get('[data-testid="step-3"] [data-testid="step-down"]').attributes('disabled')).toBeDefined();
    expect(wrapper.get('[data-testid="recorder-save"]').attributes('disabled')).toBeDefined();
  });

  it('renames, changes, moves and deletes steps, then saves the recording', async () => {
    const { wrapper, asked } = await mounted();
    await pick(wrapper, 'login');
    await wrapper.get('[data-testid="step-1"] [data-testid="step-name"]').setValue('Open the shop');
    await wrapper.get('[data-testid="step-3"] [data-testid="step-value"]').setValue('grace@shop.test');
    await wrapper.get('[data-testid="step-3"] [data-testid="step-up"]').trigger('click');
    await settle();
    await wrapper.get('[data-testid="step-2"] [data-testid="step-remove"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="recorder-dirty"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="recorder-export"]').attributes('disabled')).toBeDefined();
    await wrapper.get('[data-testid="recorder-save"]').trigger('click');
    await settle();
    const saved = asked.find((message) => message.type === 'saveRecording');
    expect(saved).toBeDefined();
    if (saved?.type !== 'saveRecording') {
      return;
    }
    expect(saved.name).toBe('login');
    expect(saved.recording.steps.map((step) => step.id)).toEqual([1, 3]);
    expect(saved.recording.steps[0]?.name).toBe('Open the shop');
    expect(saved.recording.steps[1]?.action).toEqual({
      type: 'fill',
      target: css('#email'),
      value: 'grace@shop.test',
    });
    expect(wrapper.get('[data-testid="recorder-notice"]').text()).toBe('"login" saved.');
    expect(wrapper.find('[data-testid="recorder-dirty"]').exists()).toBe(false);
  });

  it('keeps the edits and says why when the core refuses the save', async () => {
    const { wrapper } = await mounted(LIST, true);
    await pick(wrapper, 'login');
    await wrapper.get('[data-testid="step-1"] [data-testid="step-remove"]').trigger('click');
    await wrapper.get('[data-testid="recorder-save"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="recorder-error"]').text()).toContain('two steps have the same id');
    expect(wrapper.find('[data-testid="recorder-dirty"]').exists()).toBe(true);
  });

  it('lists again when the core says the recordings changed', async () => {
    const { wrapper, push, answerNext } = await mounted();
    answerNext({ ...LIST, recordings: [{ name: 'signup', steps: 2, modifiedMs: 1763381000000 }, ...LIST.recordings] });
    push({ type: 'recordingsChanged' });
    await settle();
    expect(wrapper.find('[data-testid="recording-signup"]').exists()).toBe(true);
  });

  it('exports: a highlighted preview per format and name, copy and save', async () => {
    const { wrapper, asked, copied } = await mounted();
    await pick(wrapper, 'login');
    await wrapper.get('[data-testid="recorder-export"]').trigger('click');
    await settle();
    expect(asked).toContainEqual({ type: 'exportRecording', name: 'login', format: 'flow', exportName: 'login', write: false });
    const file = wrapper.get('[data-testid="export-file-login.ts"]');
    expect(file.find('[data-kind="comment"]').text()).toBe('// Step 1: navigate');
    expect(file.find('[data-kind="keyword"]').text()).toBe('await');
    expect(file.find('[data-kind="string"]').text()).toBe("'/'");
    await wrapper.get('[data-testid="export-name"]').setValue('login-mobile');
    await settle();
    expect(asked).toContainEqual({
      type: 'exportRecording',
      name: 'login',
      format: 'flow',
      exportName: 'login-mobile',
      write: false,
    });
    await file.get('[data-testid="export-copy"]').trigger('click');
    await settle();
    expect(copied).toEqual([EXPORT.files[0]?.content]);
    await wrapper.get('[data-testid="export-save"]').trigger('click');
    await settle();
    expect(asked).toContainEqual({
      type: 'exportRecording',
      name: 'login',
      format: 'flow',
      exportName: 'login-mobile',
      write: true,
    });
    // RB-090-66: the dialog closes; the panel says where the files went.
    expect(wrapper.find('[data-testid="export-save"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="recorder-notice"]').text()).toBe('Saved in .rbrowser/out/exports/login.');
  });

  it('says when no project is open', async () => {
    const { wrapper } = await mounted({ project: false, recordings: [] });
    expect(wrapper.find('[data-testid="recorder-no-project"]').exists()).toBe(true);
  });
});
