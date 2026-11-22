import { describe, expect, it } from 'vitest';
import {
  AUTH_COMMAND,
  SETTINGS_KEY,
  canRemove,
  draftProblem,
  removeMessage,
  saveMessage,
  settingsMessage,
  storedText,
  storedUsers,
  usersOf,
} from './basicAuth';

describe('the Basic auth draft', () => {
  it('needs a user without a colon and a password, neither with a control character', () => {
    expect(draftProblem({ environment: 'staging', user: 'ada', password: 'pw-1' })).toBeNull();
    expect(draftProblem({ environment: 'staging', user: 'ada', password: 'pw1' })).toBe('auth.bar.problem.passwordShort');
    expect(draftProblem({ environment: 'staging', user: '', password: 'pw' })).toBe('auth.bar.problem.user');
    expect(draftProblem({ environment: 'staging', user: 'ada:x', password: 'pw' })).toBe('auth.bar.problem.user');
    expect(draftProblem({ environment: 'staging', user: 'ada', password: '' })).toBe('auth.bar.problem.passwordShort');
    expect(draftProblem({ environment: 'staging', user: 'ada', password: 'ab\ncd' })).toBe('auth.bar.problem.password');
    expect(draftProblem({ environment: '', user: 'ada', password: 'pw' })).toBe('auth.bar.problem.environment');
  });

  it('saves and removes through environment.auth, and links the setting', () => {
    expect(saveMessage({ environment: 'staging', user: 'ada', password: 'pw' })).toEqual({
      type: 'runCommand',
      id: AUTH_COMMAND,
      args: { environment: 'staging', user: 'ada', password: 'pw' },
    });
    expect(removeMessage('staging')).toEqual({
      type: 'runCommand',
      id: AUTH_COMMAND,
      args: { environment: 'staging', remove: true },
    });
    expect(settingsMessage()).toEqual({ type: 'runCommand', id: 'settings.open', args: { key: SETTINGS_KEY } });
  });
});

// RB-094-29: what is stored for an environment - the user, never a password.
describe('the stored credentials', () => {
  const statuses = [
    { environment: 'local', stored: false },
    { environment: 'staging', stored: true, user: 'ada' },
  ];

  it('maps each environment to its stored user or null', () => {
    expect(storedUsers(statuses)).toEqual({ local: null, staging: 'ada' });
  });

  it('says who is stored, or that nothing is, and nothing while unknown', () => {
    const users = storedUsers(statuses);
    expect(storedText(users, 'staging')).toEqual({ key: 'auth.status.stored', params: { user: 'ada' } });
    expect(storedText(users, 'local')).toEqual({ key: 'auth.status.none', params: {} });
    expect(storedText(users, 'preview')).toEqual({ key: 'auth.status.none', params: {} });
    expect(storedText(null, 'staging')).toBeNull();
  });

  it('offers Remove only with a stored credential, and while unknown', () => {
    const users = storedUsers(statuses);
    expect(canRemove(users, 'staging')).toBe(true);
    expect(canRemove(users, 'local')).toBe(false);
    expect(canRemove(null, 'local')).toBe(true);
  });

  it('takes the users from a pushed status and ignores every other message', () => {
    expect(usersOf({ type: 'basicAuthStatus', environments: statuses })).toEqual({ local: null, staging: 'ada' });
    expect(usersOf({ type: 'ack' })).toBeNull();
  });
});
