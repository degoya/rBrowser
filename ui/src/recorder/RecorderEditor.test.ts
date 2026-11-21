// The Recorder panel's recording editor (RB-092-01): a step added at a `+` with "Pick in view",
// a step tried, played from, opened in the editor and switched off - by its buttons and by a
// shortcut's command - and all of it saved with the draft.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, RecordingList, ShellToCore } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import type { Recording } from './recording';
import RecorderPage from './RecorderPage.vue';

const css = (path: string) => ({ selectors: [{ kind: 'css' as const, path }] });
const FILTER = { selectors: [{ kind: 'role' as const, role: 'button', name: 'Filter' }, { kind: 'css' as const, path: 'aside > button' }] };

function search(): Recording {
  return {
    version: 1,
    name: 'search',
    breakpoints: [
      { id: 'mobile', width: 390, height: 844, deviceScaleFactor: 3 },
      { id: 'desktop', width: 1440, height: 900, deviceScaleFactor: 1 },
    ],
    startedAtMs: 1,
    appVersion: '0.9.2',
    steps: [
      { id: 1, action: { type: 'navigate', url: 'https://example.test/' }, view: 'desktop', atMs: 0 },
      { id: 2, action: { type: 'click', target: css('#accept') }, view: 'desktop', atMs: 300 },
      { id: 3, action: { type: 'fill', target: css('#q'), value: 'running shoes' }, view: 'desktop', atMs: 900 },
    ],
  };
}

const LIST: RecordingList = {
  project: true,
  folder: '.rbrowser/out/recordings',
  recordings: [{ name: 'search', steps: 3, modifiedMs: 1763380900000 }],
};

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

async function opened() {
  const listeners = new Set<(message: CoreToShell) => void>();
  const push = (message: CoreToShell) => listeners.forEach((listener) => listener(message));
  const asked: ShellToCore[] = [];
  const client = new IpcClient({
    send: async (message) => {
      asked.push(message);
      switch (message.type) {
        case 'getRecordings':
        case 'saveRecording':
          return { type: 'recordings', ...LIST };
        case 'getRecording':
          return { type: 'recording', name: message.name, recording: search() };
        case 'runCommand':
          if (message.id === 'recorder.pick_element') {
            push({ type: 'recorderElementPicked', view: 2, locator: FILTER, text: 'Filter', onlyIn: ['desktop'] });
          }
          return { type: 'ack' };
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
  return { wrapper, asked, push };
}

function commands(asked: ShellToCore[]) {
  return asked.flatMap((message) => (message.type === 'runCommand' ? [{ id: message.id, args: message.args }] : []));
}

async function saved(wrapper: VueWrapper, asked: ShellToCore[]): Promise<Recording | undefined> {
  await wrapper.get('[data-testid="recorder-save"]').trigger('click');
  await settle();
  const message = asked.find((known) => known.type === 'saveRecording');
  return message?.type === 'saveRecording' ? message.recording : undefined;
}

describe('the recording editor (RB-092-01)', () => {
  it('adds a picked click after a step, with the pause of the setting', async () => {
    const { wrapper, asked } = await opened();
    expect(wrapper.find('[data-testid="add-step-card"]').exists()).toBe(false);
    await wrapper.get('[data-testid="step-insert-1"]').trigger('click');
    await settle();
    const card = wrapper.get('[data-testid="add-step-card"]');
    expect(card.get('[data-testid="step-add-confirm"]').attributes('disabled')).toBeDefined();
    expect(card.find('[data-testid="step-editor-missing"]').exists()).toBe(true);
    await card.get('[data-testid="locator-pick"]').trigger('click');
    await settle();
    expect(commands(asked)).toContainEqual({ id: 'recorder.pick_element', args: { recordingId: 'search' } });
    expect(wrapper.get('[data-testid="locator-candidates"]').text()).toContain('button "Filter"');
    expect(wrapper.find('[data-testid="locator-picked-in"]').exists()).toBe(true);
    await wrapper.get('[data-testid="step-add-confirm"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="add-step-card"]').exists()).toBe(false);
    const recording = await saved(wrapper, asked);
    expect(recording?.steps.map((step) => step.id)).toEqual([1, 4, 2, 3]);
    expect(recording?.steps[1]).toEqual({
      id: 4,
      action: { type: 'click', target: FILTER },
      view: 'desktop',
      atMs: 0,
      pauseMs: 500,
      onlyIn: ['desktop'],
    });
  });

  it('tries a step from the saved recording, and from the draft once it changed', async () => {
    const { wrapper, asked } = await opened();
    await wrapper.get('[data-testid="step-2"] [data-testid="step-try"]').trigger('click');
    await settle();
    expect(commands(asked)).toContainEqual({ id: 'recorder.step_try', args: { recordingId: 'search', stepId: 2 } });
    await wrapper.get('[data-testid="step-1"] [data-testid="step-name"]').setValue('Open the start');
    await wrapper.get('[data-testid="step-2"] [data-testid="step-try"]').trigger('click');
    await settle();
    const tried = commands(asked).filter((command) => command.id === 'recorder.step_try');
    expect(tried).toHaveLength(2);
    const args = tried[1]?.args as { draft?: Recording } | undefined;
    expect(args?.draft?.steps[0]?.name).toBe('Open the start');
  });

  it('opens one step in the editor, pressed while open', async () => {
    const { wrapper } = await opened();
    const edit = wrapper.get('[data-testid="step-3"] [data-testid="step-edit"]');
    expect(edit.attributes('aria-pressed')).toBe('false');
    await edit.trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="step-3"] [data-testid="step-edit"]').attributes('aria-pressed')).toBe('true');
    expect(wrapper.find('[data-testid="step-3"] [data-testid="step-editor"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="step-3"] [data-testid="step-name"]').exists()).toBe(false);
    await wrapper.get('[data-testid="step-1"] [data-testid="step-edit"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="step-3"] [data-testid="step-editor"]').exists()).toBe(false);
    await wrapper.get('[data-testid="step-editor-done"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="step-editor"]').exists()).toBe(false);
  });

  it('runs a shortcut on the step in focus: skip it, play from it', async () => {
    const { wrapper, asked, push } = await opened();
    await wrapper.get('[data-testid="step-2"] [data-testid="step-up"]').trigger('focusin');
    push({ type: 'recorderStepCommand', command: 'recorder.step_toggle' });
    await settle();
    expect(wrapper.get('[data-testid="step-2"]').attributes('data-disabled')).toBe('true');
    expect(wrapper.find('[data-testid="step-2"] [data-testid="step-skipped"]').exists()).toBe(true);
    push({ type: 'recorderStepCommand', command: 'recorder.play_from' });
    await settle();
    expect(commands(asked)).toContainEqual(
      expect.objectContaining({ id: 'recorder.play_from', args: expect.objectContaining({ recordingId: 'search', stepId: 2 }) }),
    );
    // Continuing waits while the draft has unsaved edits.
    push({ type: 'recorderStepCommand', command: 'recorder.continue_from' });
    await settle();
    expect(commands(asked).some((command) => command.id === 'recorder.continue_from')).toBe(false);
    const recording = await saved(wrapper, asked);
    expect(recording?.steps[1]?.disabled).toBe(true);
  });

  it('adds a step at the end by its shortcut when no step has the focus', async () => {
    const { wrapper, push } = await opened();
    push({ type: 'recorderStepCommand', command: 'recorder.step_add' });
    await settle();
    const gaps = wrapper.findAll('[data-testid="add-step-card"]');
    expect(gaps).toHaveLength(1);
    // The card follows the last step.
    const html = wrapper.get('[data-testid="recorder-steps"]').html();
    expect(html.indexOf('add-step-card')).toBeGreaterThan(html.indexOf('data-testid="step-3"'));
  });

  it('continues the saved recording from a step', async () => {
    const { wrapper, asked, push } = await opened();
    await wrapper.get('[data-testid="step-1"] [data-testid="step-try"]').trigger('focusin');
    push({ type: 'recorderStepCommand', command: 'recorder.continue_from' });
    await settle();
    expect(commands(asked)).toContainEqual({ id: 'recorder.continue_from', args: { recordingId: 'search', stepId: 1 } });
  });
});
