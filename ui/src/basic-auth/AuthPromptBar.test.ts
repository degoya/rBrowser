// RB-099-04: rBrowser's own sign-in dialog in the shell's strip - one question per origin, realm
// and proxy, answered once for every view waiting, or cancelled.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { CoreToShell } from '../ipc/protocol';
import { mountWithShell, settle } from '../test/mountShell';
import AuthPromptBar from './AuthPromptBar.vue';

let current: VueWrapper | null = null;
afterEach(() => {
  current?.unmount();
  current = null;
});

const SITE: CoreToShell = {
  type: 'authPrompt',
  id: 4,
  origin: 'https://dev.example.test:443',
  realm: 'Members',
  proxy: false,
  views: [1, 3],
  environment: 'development',
  refused: false,
  insecure: false,
};

async function mounted() {
  let push: (message: CoreToShell) => void = () => undefined;
  const subscribe = (listener: (message: CoreToShell) => void) => {
    push = listener;
    return () => undefined;
  };
  const result = await mountWithShell(AuthPromptBar, { mount: { props: { subscribe } } });
  current = result.wrapper;
  const send = async (message: CoreToShell) => {
    push(message);
    await settle();
  };
  return { ...result, send };
}

const replies = (sent: { type: string }[]) => sent.filter((message) => message.type === 'authPromptReply');
const bar = (wrapper: VueWrapper) => wrapper.find('[data-testid="auth-prompt"]');

describe('the sign-in dialog', () => {
  it('stays hidden until the core asks, then names origin, realm and the views waiting', async () => {
    const { wrapper, send } = await mounted();
    expect(bar(wrapper).exists()).toBe(false);
    await send(SITE);
    expect(wrapper.get('[data-testid="auth-prompt-title"]').text()).toBe('Sign in to https://dev.example.test:443');
    expect(wrapper.get('[data-testid="auth-prompt-realm"]').text()).toBe('Realm: Members');
    expect(wrapper.get('[data-testid="auth-prompt-views"]').text()).toBe('Waiting: Mobile, Desktop');
    expect(wrapper.find('[data-testid="auth-prompt-refused"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="auth-prompt-save"]').attributes('role')).toBe('checkbox');
    expect(wrapper.text()).toContain('Save for environment development');
    expect((wrapper.get('[data-testid="auth-prompt-password"]').element as HTMLInputElement).type).toBe('password');
  });

  it('answers every view at once with the credential, used once, and forgets the password', async () => {
    const { wrapper, send, mock } = await mounted();
    await send(SITE);
    await wrapper.get('[data-testid="auth-prompt-user"]').setValue('ada');
    await wrapper.get('[data-testid="auth-prompt-password"]').setValue('dialog-test-password');
    await bar(wrapper).trigger('submit');
    await settle();
    expect(replies(mock.sent)).toEqual([
      {
        type: 'authPromptReply',
        id: 4,
        signIn: { user: 'ada', password: 'dialog-test-password', save: false },
      },
    ]);
    expect(bar(wrapper).exists()).toBe(false);
    expect(wrapper.html()).not.toContain('dialog-test-password');
  });

  it('saves for the environment only when asked, under the store rules', async () => {
    const { wrapper, send, mock } = await mounted();
    await send(SITE);
    await wrapper.get('[data-testid="auth-prompt-save"]').trigger('click');
    await wrapper.get('[data-testid="auth-prompt-user"]').setValue('ada');
    await wrapper.get('[data-testid="auth-prompt-password"]').setValue('pw');
    await bar(wrapper).trigger('submit');
    await settle();
    expect(replies(mock.sent)).toEqual([]);
    expect(wrapper.get('[data-testid="auth-prompt-problem"]').text()).toContain('4');
    await wrapper.get('[data-testid="auth-prompt-password"]').setValue('dialog-save-password');
    await bar(wrapper).trigger('submit');
    await settle();
    expect(replies(mock.sent)).toEqual([
      { type: 'authPromptReply', id: 4, signIn: { user: 'ada', password: 'dialog-save-password', save: true } },
    ]);
  });

  it('is a named form, not a modal dialog, whose fields a password manager may fill (RB-099-21)', async () => {
    const { wrapper, send } = await mounted();
    await send(SITE);
    const form = bar(wrapper);
    expect(form.attributes('role')).toBe('form');
    expect(form.attributes('aria-label')).toBe('Sign in to https://dev.example.test:443');
    expect(form.attributes('aria-modal')).toBeUndefined();
    expect(wrapper.get('[data-testid="auth-prompt-user"]').attributes('autocomplete')).toBe('username');
    expect(wrapper.get('[data-testid="auth-prompt-password"]').attributes('autocomplete')).toBe('current-password');
  });

  it('says a problem assertively and ties it to the field it is about (RB-099-21)', async () => {
    const { wrapper, send } = await mounted();
    await send(SITE);
    const region = wrapper.get('[data-testid="auth-prompt-problem"]');
    // The alert region is in the DOM before its first word.
    expect(region.attributes('role')).toBe('alert');
    expect(region.text()).toBe('');
    const user = wrapper.get('[data-testid="auth-prompt-user"]');
    const password = wrapper.get('[data-testid="auth-prompt-password"]');
    expect(password.attributes('aria-invalid')).toBeUndefined();
    await wrapper.get('[data-testid="auth-prompt-save"]').trigger('click');
    await user.setValue('ada');
    await password.setValue('pw');
    await bar(wrapper).trigger('submit');
    await settle();
    const id = region.attributes('id') ?? '';
    expect(id).not.toBe('');
    expect(region.text()).toContain('4');
    expect(password.attributes('aria-invalid')).toBe('true');
    expect(password.attributes('aria-describedby')).toBe(id);
    expect(user.attributes('aria-invalid')).toBeUndefined();
    expect(user.attributes('aria-describedby')).toBeUndefined();
  });

  it('offers no save for a proxy or another site, and says when the server refused', async () => {
    const { wrapper, send } = await mounted();
    await send({ ...SITE, id: 5, origin: 'proxy.example.test:3128', proxy: true, realm: undefined, environment: undefined, refused: true });
    expect(wrapper.get('[data-testid="auth-prompt-title"]').text()).toBe(
      'Sign in to the proxy proxy.example.test:3128',
    );
    expect(wrapper.find('[data-testid="auth-prompt-save"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="auth-prompt-realm"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="auth-prompt-refused"]').text()).toContain('refused');
  });

  it('says when the answer would cross the network unencrypted (review w35)', async () => {
    const { wrapper, send } = await mounted();
    await send(SITE);
    expect(wrapper.find('[data-testid="auth-prompt-insecure"]').exists()).toBe(false);
    await send({ ...SITE, id: 8, origin: 'http://plain.example.test:80', environment: undefined, insecure: true });
    await send({ type: 'authPromptClosed', id: 4 });
    expect(wrapper.get('[data-testid="auth-prompt-title"]').text()).toBe('Sign in to http://plain.example.test:80');
    expect(wrapper.get('[data-testid="auth-prompt-insecure"]').text()).toContain('unencrypted');
  });

  it('cancels with the button or Escape, without a credential', async () => {
    const { wrapper, send, mock } = await mounted();
    await send(SITE);
    await wrapper.get('[data-testid="auth-prompt-user"]').setValue('ada');
    await bar(wrapper).trigger('keydown', { key: 'Escape' });
    await settle();
    expect(replies(mock.sent)).toEqual([{ type: 'authPromptReply', id: 4 }]);
    expect(bar(wrapper).exists()).toBe(false);
    await send({ ...SITE, id: 6 });
    await wrapper.get('[data-testid="auth-prompt-cancel"]').trigger('click');
    await settle();
    expect(replies(mock.sent).at(-1)).toEqual({ type: 'authPromptReply', id: 6 });
  });

  it('queues questions, follows the views of an open one and drops a withdrawn one', async () => {
    const { wrapper, send } = await mounted();
    await send(SITE);
    await send({ ...SITE, id: 7, origin: 'https://other.example.test:443', environment: undefined });
    expect(wrapper.get('[data-testid="auth-prompt-queued"]').text()).toBe('1 more waiting');
    await send({ ...SITE, views: [1, 2, 3] });
    expect(wrapper.get('[data-testid="auth-prompt-views"]').text()).toBe('Waiting: Mobile, Tablet, Desktop');
    await send({ type: 'authPromptClosed', id: 4 });
    expect(wrapper.get('[data-testid="auth-prompt-title"]').text()).toBe('Sign in to https://other.example.test:443');
    expect(wrapper.find('[data-testid="auth-prompt-queued"]').exists()).toBe(false);
    await send({ type: 'authPromptClosed', id: 7 });
    expect(bar(wrapper).exists()).toBe(false);
  });
});
