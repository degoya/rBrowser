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
    // The sessions are buttons as in the assign mode (RB-080-88: a select's popover would be cut
    // off by the strip), the current one chosen and focused.
    const choices = wrapper.get('[data-testid="session-clear-select"]');
    expect(choices.attributes('role')).toBe('group');
    expect(choices.attributes('aria-labelledby')).toBeDefined();
    const shared = wrapper.get('[data-testid="session-clear-option-default"]');
    expect(shared.attributes('aria-pressed')).toBe('true');
    expect(document.activeElement).toBe(shared.element);
    await wrapper.get('[data-testid="session-clear-option-admin"]').trigger('click');
    expect(wrapper.get('[data-testid="session-clear-option-admin"]').attributes('aria-pressed')).toBe('true');
    expect(shared.attributes('aria-pressed')).toBe('false');
    expect(wrapper.get('[data-testid="session-clear-confirm"]').text()).toContain('"admin"');
    expect(mock.sent.length).toBe(before);
    await wrapper.get('[data-testid="session-clear-action"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'session.clear', args: { session: 'admin' } });
  });

  // RB-090-63 review: from the page menu the core names the clicked view's session; the bar
  // starts there and still asks before anything is cleared.
  it('starts the confirmation at the session the core names', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'session.clear', args: { session: 'admin' } });
    await settle();
    const before = mock.sent.length;
    const admin = wrapper.get('[data-testid="session-clear-option-admin"]');
    expect(admin.attributes('aria-pressed')).toBe('true');
    expect(document.activeElement).toBe(admin.element);
    expect(mock.sent.length).toBe(before);
  });

  it('ignores a named session the project does not know', async () => {
    const { wrapper, push } = await mounted();
    push({ type: 'uiCommand', id: 'session.clear', args: { session: 'gone' } });
    await settle();
    expect(wrapper.get('[data-testid="session-clear-option-default"]').attributes('aria-pressed')).toBe('true');
  });

  it('is drawn with Nuxt UI buttons and focuses the first session to assign (RB-080-88)', async () => {
    const { wrapper, push } = await mounted();
    push({ type: 'uiCommand', id: 'session.assign' });
    await settle();
    const first = wrapper.get('[data-testid="session-option-default"]');
    expect(first.attributes('data-slot')).toBe('base');
    expect(document.activeElement).toBe(first.element);
    expect(wrapper.get('[data-testid="session-cancel"]').attributes('data-slot')).toBe('base');
    push({ type: 'uiCommand', id: 'session.clear' });
    await settle();
    expect(wrapper.get('[data-testid="session-clear-action"]').attributes('data-slot')).toBe('base');
    expect(wrapper.find('select').exists()).toBe(false);
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
