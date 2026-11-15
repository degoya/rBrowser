import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, ShellToCore } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import FindingsPage from './FindingsPage.vue';
import { PANEL_COMMAND, findingsChannel } from './host';
import { sampleMessages, sampleProgress, sampleRun } from './testing';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

/** The page in its window, over a client whose `getFindings` answers `last`. */
async function mounted(last: CoreToShell = { type: 'ack' }) {
  const listeners = new Set<(message: CoreToShell) => void>();
  const subscribe = (listener: (message: CoreToShell) => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };
  const asked: ShellToCore[] = [];
  const client = new IpcClient({
    send: async (message) => {
      asked.push(message);
      return message.type === 'getFindings' ? last : { type: 'ack' };
    },
    subscribe: () => () => undefined,
  });
  const result = await mountWithShell(FindingsPage, { mount: { props: { subscribe, client } } });
  current = result.wrapper;
  await settle();
  const push = (message: CoreToShell) => listeners.forEach((listener) => listener(message));
  return { ...result, push, asked };
}

describe('the findings page', () => {
  it('fills its window with the panel and puts the focus on Run', async () => {
    const { wrapper } = await mounted();
    expect(wrapper.find('[data-testid="findings-panel"]').exists()).toBe(true);
    expect(document.activeElement).toBe(wrapper.get('[data-testid="run"]').element);
  });

  it('shows the run that ended before the window opened', async () => {
    const { wrapper, asked } = await mounted({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    expect(asked).toContainEqual({ type: 'getFindings' });
    expect(wrapper.find('[data-testid="finding-a"]').text()).toContain('2 images without alt text');
  });

  it('shows a run that ends while it is open', async () => {
    const { wrapper, push } = await mounted();
    push({ type: 'findings', run: sampleRun(), messages: sampleMessages });
    await settle();
    expect(wrapper.find('[data-testid="finding-a"]').text()).toContain('2 images without alt text');
  });

  it('sends the panel requests to the core', async () => {
    const { wrapper, mock } = await mounted();
    await wrapper.get('[data-testid="run"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runChecks', target: 'all' });
  });
});

describe('the findings channel', () => {
  it('rejects a refused request and passes only findings events', async () => {
    const sent: ShellToCore[] = [];
    let accept = true;
    const send = async (message: ShellToCore) => {
      sent.push(message);
      return accept;
    };
    let push: (message: CoreToShell) => void = () => undefined;
    const channel = findingsChannel(send, (listener) => {
      push = listener;
      return () => undefined;
    });
    await expect(channel.request({ type: 'runChecks', target: 'all' })).resolves.toBeUndefined();
    accept = false;
    await expect(
      channel.request({ type: 'highlightFinding', findingId: 'a', selector: 'img', views: [1] }),
    ).rejects.toThrow('highlightFinding');
    expect(sent).toHaveLength(2);

    const seen: string[] = [];
    channel.on((event) => seen.push(event.type === 'findings' ? event.run.runId : `progress ${event.runId}`));
    push({ type: 'uiCommand', id: PANEL_COMMAND });
    push({ type: 'findings', run: sampleRun(), messages: {} });
    push(sampleProgress());
    expect(seen).toEqual(['run-0000000000001', 'progress run-0000000000002']);
  });

  it('asks for the last run only when it can, and takes only a run', async () => {
    const none = findingsChannel(async () => true, () => () => undefined);
    await expect(none.last()).resolves.toBeNull();
    const ack = findingsChannel(async () => true, () => () => undefined, async () => ({ type: 'ack' }));
    await expect(ack.last()).resolves.toBeNull();
    const run = findingsChannel(
      async () => true,
      () => () => undefined,
      async () => ({ type: 'findings', run: sampleRun(), messages: {} }),
    );
    await expect(run.last()).resolves.toMatchObject({ type: 'findings' });
    await expect(run.progress()).resolves.toBeNull();
    const going = findingsChannel(
      async () => true,
      () => () => undefined,
      async (message) => (message.type === 'getChecksProgress' ? sampleProgress() : { type: 'ack' }),
    );
    await expect(going.progress()).resolves.toMatchObject({ type: 'checksProgress', runId: 'run-0000000000002' });
    await expect(none.progress()).resolves.toBeNull();
  });
});
