import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CoreToShell } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import ExtensionPromptBar from './ExtensionPromptBar.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const UPDATE: CoreToShell = {
  type: 'extensionPrompt',
  promptId: 9,
  kind: 'update',
  id: 'nhdogjmejiglipccpnnnanhbledajbpd',
  name: 'Vue.js devtools',
  version: '7.6.4',
  currentVersion: '7.6.0',
};
const REACT: CoreToShell = {
  ...UPDATE,
  promptId: 10,
  id: 'fmkadmapgofadopljbjfkapdkoienihi',
  name: 'React Developer Tools',
  version: '6.1.1',
};

async function mounted() {
  let push: (message: CoreToShell) => void = () => undefined;
  const subscribe = (listener: (message: CoreToShell) => void) => {
    push = listener;
    return () => undefined;
  };
  const result = await mountWithShell(ExtensionPromptBar, { mount: { props: { subscribe } } });
  current = result.wrapper;
  const open = async (message: CoreToShell = UPDATE) => {
    push(message);
    await settle();
  };
  return { ...result, open };
}

describe('the extension prompt bar', () => {
  it('stays hidden until the core asks, then names the extension and the versions', async () => {
    const { wrapper, open } = await mounted();
    expect(wrapper.find('[data-testid="extension-prompt"]').exists()).toBe(false);
    await open();
    const text = wrapper.get('[data-testid="extension-prompt-text"]').text();
    expect(text).toContain('Vue.js devtools');
    expect(text).toContain('7.6.4');
    expect(text).toContain('7.6.0');
    expect(wrapper.get('[data-testid="extension-prompt-accept"]').text()).toBe('Update');
  });

  it('answers the prompt with its id and closes', async () => {
    const { wrapper, open, mock } = await mounted();
    await open();
    await wrapper.get('[data-testid="extension-prompt-accept"]').trigger('click');
    await settle();
    expect(mock.sent.filter((m) => m.type === 'extensionDecision')).toEqual([
      { type: 'extensionDecision', promptId: 9, accepted: true },
    ]);
    expect(wrapper.find('[data-testid="extension-prompt"]').exists()).toBe(false);
  });

  it('declines with the button or Escape', async () => {
    const { wrapper, open, mock } = await mounted();
    await open({ ...UPDATE, kind: 'install', currentVersion: undefined });
    expect(wrapper.get('[data-testid="extension-prompt-accept"]').text()).toBe('Install');
    await wrapper.get('[data-testid="extension-prompt"]').trigger('keydown', { key: 'Escape' });
    await settle();
    expect(mock.sent.filter((m) => m.type === 'extensionDecision')).toEqual([
      { type: 'extensionDecision', promptId: 9, accepted: false },
    ]);
    expect(wrapper.find('[data-testid="extension-prompt"]').exists()).toBe(false);
  });

  it('names a removal and offers Remove (RB-080-30)', async () => {
    const { wrapper, open, mock } = await mounted();
    await open({ ...UPDATE, kind: 'remove', currentVersion: undefined });
    const text = wrapper.get('[data-testid="extension-prompt-text"]').text();
    expect(text).toContain('remove');
    expect(text).toContain('Vue.js devtools');
    expect(text).toContain('7.6.4');
    expect(wrapper.get('[data-testid="extension-prompt-accept"]').text()).toBe('Remove');
    await wrapper.get('[data-testid="extension-prompt-accept"]').trigger('click');
    await settle();
    expect(mock.sent.filter((m) => m.type === 'extensionDecision')).toEqual([
      { type: 'extensionDecision', promptId: 9, accepted: true },
    ]);
  });

  it('queues a second prompt and shows it after the first is answered', async () => {
    const { wrapper, open, mock } = await mounted();
    await open();
    await open(REACT);
    expect(wrapper.get('[data-testid="extension-prompt-text"]').text()).toContain('Vue.js devtools');
    await wrapper.get('[data-testid="extension-prompt-accept"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="extension-prompt-text"]').text()).toContain('React Developer Tools');
    await wrapper.get('[data-testid="extension-prompt-decline"]').trigger('click');
    await settle();
    expect(mock.sent.filter((m) => m.type === 'extensionDecision')).toEqual([
      { type: 'extensionDecision', promptId: 9, accepted: true },
      { type: 'extensionDecision', promptId: 10, accepted: false },
    ]);
    expect(wrapper.find('[data-testid="extension-prompt"]').exists()).toBe(false);
  });

  it('replaces the prompt of the same extension in place, as the core forgets the earlier one', async () => {
    const { wrapper, open, mock } = await mounted();
    await open();
    await open(REACT);
    await open({ ...UPDATE, promptId: 11, version: '7.6.5' });
    expect(wrapper.get('[data-testid="extension-prompt-text"]').text()).toContain('7.6.5');
    await wrapper.get('[data-testid="extension-prompt-accept"]').trigger('click');
    await settle();
    expect(wrapper.get('[data-testid="extension-prompt-text"]').text()).toContain('React Developer Tools');
    await wrapper.get('[data-testid="extension-prompt-accept"]').trigger('click');
    await settle();
    const answered = mock.sent.filter((m) => m.type === 'extensionDecision').map((m) => m.promptId);
    expect(answered).toEqual([11, 10]);
    expect(wrapper.find('[data-testid="extension-prompt"]').exists()).toBe(false);
  });
});
