// Basic auth per environment in the shell (RB-094-08): what the bar sends. The password goes to
// the core once, inside `environment.auth`, which keeps it in the platform's credential store;
// the shell never stores it and never shows one back.

import type { ShellToCore } from '../ipc/protocol';

export const AUTH_COMMAND = 'environment.auth';
/** The setting that lists the environments behind Basic auth (Project > Environments). */
export const SETTINGS_KEY = 'basicAuth.environments';

/** What the bar holds while the person types. */
export interface AuthDraft {
  environment: string;
  user: string;
  password: string;
}

/** The core refuses a shorter password: its log masking skips values below 4 characters. */
export const MIN_PASSWORD = 4;

// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/u;

/** Why `draft` cannot be saved, as an i18n key; `null` when it can. */
export function draftProblem(draft: AuthDraft): string | null {
  if (draft.environment === '') {
    return 'auth.bar.problem.environment';
  }
  if (draft.user === '' || draft.user.includes(':') || CONTROL.test(draft.user)) {
    return 'auth.bar.problem.user';
  }
  if ([...draft.password].length < MIN_PASSWORD) {
    return 'auth.bar.problem.passwordShort';
  }
  if (CONTROL.test(draft.password)) {
    return 'auth.bar.problem.password';
  }
  return null;
}

/** `environment.auth` storing the credential of `draft`. */
export function saveMessage(draft: AuthDraft): ShellToCore {
  return {
    type: 'runCommand',
    id: AUTH_COMMAND,
    args: { environment: draft.environment, user: draft.user, password: draft.password },
  };
}

/** `environment.auth` removing the credential of `environment`. */
export function removeMessage(environment: string): ShellToCore {
  return { type: 'runCommand', id: AUTH_COMMAND, args: { environment, remove: true } };
}

/** Opens Settings at the list of environments behind Basic auth. */
export function settingsMessage(): ShellToCore {
  return { type: 'runCommand', id: 'settings.open', args: { key: SETTINGS_KEY } };
}
