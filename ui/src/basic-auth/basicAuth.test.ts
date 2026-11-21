import { describe, expect, it } from 'vitest';
import { AUTH_COMMAND, SETTINGS_KEY, draftProblem, removeMessage, saveMessage, settingsMessage } from './basicAuth';

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
