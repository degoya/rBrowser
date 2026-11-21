import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CoreToShell } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import EnvironmentAuthBar from './EnvironmentAuthBar.vue';

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
  const result = await mountWithShell(EnvironmentAuthBar, { mount: { props: { subscribe } } });
  current = result.wrapper;
  return { ...result, push: (message: CoreToShell) => push(message) };
}

function passwordOf(wrapper: VueWrapper): string {
  return (wrapper.get('[data-testid="basic-auth-password"]').element as HTMLInputElement).value;
}

describe('the Basic auth bar', () => {
  it('stays closed for other commands', async () => {
    const { wrapper, push } = await mounted();
    push({ type: 'uiCommand', id: 'session.assign' });
    await settle();
    expect(wrapper.find('[data-testid="basic-auth-bar"]').exists()).toBe(false);
  });

  it('starts at the environment shown and sends the credential once, then forgets it', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'environment.auth' });
    await settle();
    expect(wrapper.get('[data-testid="basic-auth-env-local"]').attributes('aria-pressed')).toBe('true');
    await wrapper.get('[data-testid="basic-auth-env-staging"]').trigger('click');
    await wrapper.get('[data-testid="basic-auth-user"]').setValue('ada');
    await wrapper.get('[data-testid="basic-auth-password"]').setValue('bar-test-password');
    expect(passwordOf(wrapper)).toBe('bar-test-password');
    expect((wrapper.get('[data-testid="basic-auth-password"]').element as HTMLInputElement).type).toBe(
      'password',
    );
    await wrapper.get('[data-testid="basic-auth-bar"]').trigger('submit');
    await settle();
    expect(mock.sent.at(-1)).toEqual({
      type: 'runCommand',
      id: 'environment.auth',
      args: { environment: 'staging', user: 'ada', password: 'bar-test-password' },
    });
    // The mock core answers `runCommand` with 501: the bar stays open, the password is gone.
    expect(wrapper.find('[data-testid="basic-auth-bar"]').exists()).toBe(true);
    expect(passwordOf(wrapper)).toBe('');
  });

  it('sends nothing for a user with a colon or without a password', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'environment.auth' });
    await settle();
    const before = mock.sent.length;
    await wrapper.get('[data-testid="basic-auth-user"]').setValue('ada:x');
    await wrapper.get('[data-testid="basic-auth-password"]').setValue('pw');
    await wrapper.get('[data-testid="basic-auth-bar"]').trigger('submit');
    await settle();
    await wrapper.get('[data-testid="basic-auth-user"]').setValue('ada');
    await wrapper.get('[data-testid="basic-auth-password"]').setValue('');
    await wrapper.get('[data-testid="basic-auth-bar"]').trigger('submit');
    await settle();
    expect(mock.sent.length).toBe(before);
  });

  it('removes the chosen environment and empties the password on Escape', async () => {
    const { wrapper, mock, push } = await mounted();
    push({ type: 'uiCommand', id: 'environment.auth' });
    await settle();
    await wrapper.get('[data-testid="basic-auth-remove"]').trigger('click');
    await settle();
    expect(mock.sent.at(-1)).toEqual({
      type: 'runCommand',
      id: 'environment.auth',
      args: { environment: 'local', remove: true },
    });
    await wrapper.get('[data-testid="basic-auth-password"]').setValue('typed-then-left');
    await wrapper.get('[data-testid="basic-auth-bar"]').trigger('keydown', { key: 'Escape' });
    await settle();
    expect(wrapper.find('[data-testid="basic-auth-bar"]').exists()).toBe(false);
    push({ type: 'uiCommand', id: 'environment.auth' });
    await settle();
    expect(passwordOf(wrapper)).toBe('');
  });
});
