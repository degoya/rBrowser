// RB-094-24: Basic auth per environment in Settings > Project > Environments - on/off (the key
// `basicAuth.environments`), user and password, stored by `environment.auth` in the same place
// the shell's bar stores them; the password field never shows a value.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, EnvironmentAuthStatus, ShellToCore } from '../ipc/protocol';
import EnvironmentAuth from './EnvironmentAuth.vue';
import { environmentIdsOf, withListed } from './environmentAuth';
import { createFakeCore } from './test/fakeCore';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

const ENVIRONMENTS = [
  { id: 'local', url: 'http://localhost:3000' },
  { id: 'staging', url: 'https://staging.example.test' },
];

async function open(listed: string[] = []) {
  const mounted = await mountPage(EnvironmentAuth, { environments: ENVIRONMENTS, listed });
  current = mounted.wrapper;
  return mounted;
}

describe('environment auth logic', () => {
  it('reads the environment ids and skips what is no environment', () => {
    expect(environmentIdsOf([...ENVIRONMENTS, { url: 'x' }, 'y', null])).toEqual(['local', 'staging']);
    expect(environmentIdsOf(null)).toEqual([]);
  });

  it('switches one environment on or off and keeps the others', () => {
    expect(withListed(['local'], 'staging', true)).toEqual(['local', 'staging']);
    expect(withListed(['local', 'staging'], 'local', false)).toEqual(['staging']);
    expect(withListed(['local'], 'local', true)).toEqual(['local']);
    expect(withListed('not a list', 'local', true)).toEqual(['local']);
  });
});

describe('environment auth section', () => {
  it('shows each environment with its switch from basicAuth.environments', async () => {
    const { wrapper } = await open(['staging']);
    const staging = wrapper.get('[data-testid="env-auth-switch-staging"]');
    const local = wrapper.get('[data-testid="env-auth-switch-local"]');
    expect(staging.attributes('aria-checked')).toBe('true');
    expect(local.attributes('aria-checked')).toBe('false');
  });

  it('writes the list when an environment is switched on', async () => {
    const { wrapper } = await open(['staging']);
    await wrapper.get('[data-testid="env-auth-switch-local"]').trigger('click');
    await settle();
    expect(wrapper.emitted('set')?.[0]).toEqual([['staging', 'local']]);
  });

  it('stores user and password through environment.auth and clears the password', async () => {
    const { wrapper, core } = await open(['staging']);
    const password = wrapper.get('[data-testid="env-auth-password-staging"]');
    expect(password.attributes('type')).toBe('password');
    await wrapper.get('[data-testid="env-auth-user-staging"]').setValue('ada');
    await password.setValue('secret-1');
    await wrapper.get('[data-testid="env-auth-save-staging"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({
      type: 'runCommand',
      id: 'environment.auth',
      args: { environment: 'staging', user: 'ada', password: 'secret-1' },
    });
    expect((password.element as HTMLInputElement).value).toBe('');
    expect(wrapper.get('[data-testid="env-auth-status-staging"]').text()).not.toBe('');
  });

  it('refuses a short password with the reason and sends nothing', async () => {
    const { wrapper, core } = await open(['staging']);
    await wrapper.get('[data-testid="env-auth-user-staging"]').setValue('ada');
    await wrapper.get('[data-testid="env-auth-password-staging"]').setValue('pw');
    await wrapper.get('[data-testid="env-auth-save-staging"]').trigger('click');
    await settle();
    expect(core.sent.filter((message) => message.type === 'runCommand')).toEqual([]);
    expect(wrapper.get('[data-testid="env-auth-status-staging"]').text()).toContain('4');
  });

  it('removes the stored credential of one environment', async () => {
    const { wrapper, core } = await open(['staging']);
    await wrapper.get('[data-testid="env-auth-remove-staging"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({
      type: 'runCommand',
      id: 'environment.auth',
      args: { environment: 'staging', remove: true },
    });
  });
});

/** A core that knows the stored credentials: `getBasicAuthStatus` answers them, `environment.auth`
 * changes them as the Credential Manager would (RB-094-29). */
function authCore(initial: EnvironmentAuthStatus[]) {
  let statuses = structuredClone(initial);
  const sent: ShellToCore[] = [];
  const listeners = new Set<(message: CoreToShell) => void>();
  const client = new IpcClient({
    send(message) {
      sent.push(message);
      if (message.type === 'getBasicAuthStatus') {
        return Promise.resolve({ type: 'basicAuthStatus', environments: structuredClone(statuses) });
      }
      if (message.type === 'runCommand' && message.id === 'environment.auth') {
        const args = message.args as { environment: string; user?: string; remove?: boolean };
        statuses = statuses.map((status) =>
          status.environment !== args.environment
            ? status
            : args.remove === true
              ? { environment: status.environment, stored: false }
              : { environment: status.environment, stored: true, user: args.user ?? '' },
        );
        return Promise.resolve({ type: 'ack' });
      }
      return Promise.resolve({ type: 'ack' });
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  });
  const push = (message: CoreToShell): void => listeners.forEach((listener) => listener(message));
  return { client, sent, push };
}

// RB-094-29: each row says whether a credential is stored and for whom - never a password.
describe('environment auth section and the stored credentials', () => {
  const STATUSES: EnvironmentAuthStatus[] = [
    { environment: 'local', stored: false },
    { environment: 'staging', stored: true, user: 'ada' },
  ];

  async function openWith(core: ReturnType<typeof authCore>) {
    const mounted = await mountPage(EnvironmentAuth, { environments: ENVIRONMENTS, listed: ['staging'], client: core.client });
    current = mounted.wrapper;
    return mounted.wrapper;
  }

  const stored = (wrapper: VueWrapper, env: string): string => wrapper.get(`[data-testid="env-auth-stored-${env}"]`).text();
  const removeDisabled = (wrapper: VueWrapper, env: string): boolean =>
    wrapper.get(`[data-testid="env-auth-remove-${env}"]`).attributes('disabled') !== undefined;

  it('names the stored user and disables Remove without a credential', async () => {
    const wrapper = await openWith(authCore(STATUSES));
    expect(stored(wrapper, 'staging')).toBe('Credentials stored for ada');
    expect(stored(wrapper, 'local')).toBe('No credentials stored');
    expect(removeDisabled(wrapper, 'staging')).toBe(false);
    expect(removeDisabled(wrapper, 'local')).toBe(true);
  });

  it('is current after Save and after Remove', async () => {
    const core = authCore(STATUSES);
    const wrapper = await openWith(core);
    await wrapper.get('[data-testid="env-auth-user-local"]').setValue('grace');
    await wrapper.get('[data-testid="env-auth-password-local"]').setValue('secret-2');
    await wrapper.get('[data-testid="env-auth-save-local"]').trigger('click');
    await settle();
    expect(stored(wrapper, 'local')).toBe('Credentials stored for grace');
    expect(removeDisabled(wrapper, 'local')).toBe(false);
    await wrapper.get('[data-testid="env-auth-remove-staging"]').trigger('click');
    await settle();
    expect(stored(wrapper, 'staging')).toBe('No credentials stored');
    expect(removeDisabled(wrapper, 'staging')).toBe(true);
    expect(wrapper.text()).not.toContain('secret-2');
  });

  it('follows a status the core pushes after the bar stored one', async () => {
    const core = authCore(STATUSES);
    const wrapper = await openWith(core);
    core.push({
      type: 'basicAuthStatus',
      environments: [
        { environment: 'local', stored: true, user: 'linus' },
        { environment: 'staging', stored: false },
      ],
    });
    await settle();
    expect(stored(wrapper, 'local')).toBe('Credentials stored for linus');
    expect(stored(wrapper, 'staging')).toBe('No credentials stored');
  });
});
