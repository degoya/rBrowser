import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CoreToShell } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import SessionBar from './SessionBar.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

async function mounted() {
  let push: (message: CoreToShell) => void = () => undefined;
  const subscribe = (listener: (message: CoreToShell) => void) => {
    push = listener;
    return () => undefined;
  };
  const result = await mountWithShell(SessionBar, { mount: { props: { subscribe } } });
  current = result.wrapper;
  return { ...result, push: (message: CoreToShell) => push(message) };
}

describe('the session bar', () => {
  it('stays closed for other commands', async () => {
    const { wrapper, push } = await mounted();
    push({ type: 'uiCommand', id: 'view.focus_address_bar' });
    await settle();
    expect(wrapper.find('[data-testid="session-bar"]').exists()).toBe(false);
  });

  it('moves the active view into the chosen session', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'session.assign' });
    await settle();
    const current = wrapper.get('[data-testid="session-option-default"]');
    expect(current.attributes('aria-pressed')).toBe('true');
    expect(current.text()).toBe('default (shared) (current)');
    await wrapper.get('[data-testid="session-option-admin"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({
      type: 'runCommand',
      id: 'session.assign',
      args: { target: 1, session: 'admin' },
    });
    // The mock core answers `runCommand` with 501: the bar stays open, the toolbar says why.
    expect(wrapper.find('[data-testid="session-bar"]').exists()).toBe(true);
  });

  it('asks before it clears and clears the chosen session only', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'session.clear' });
    await settle();
    const before = mock.sent.length;
    await wrapper.get('[data-testid="session-clear-select"]').setValue('admin');
    expect(wrapper.get('[data-testid="session-clear-confirm"]').text()).toContain('"admin"');
    expect(mock.sent.length).toBe(before);
    await wrapper.get('[data-testid="session-clear-action"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'session.clear', args: { session: 'admin' } });
  });

  it('cancels without sending', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'session.clear' });
    await settle();
    const before = mock.sent.length;
    await wrapper.get('[data-testid="session-bar"]').trigger('keydown', { key: 'Escape' });
    await settle();
    expect(wrapper.find('[data-testid="session-bar"]').exists()).toBe(false);
    expect(mock.sent.length).toBe(before);
  });
});
