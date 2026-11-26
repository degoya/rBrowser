// Basic auth per environment in the shell (RB-094-08): what the bar sends. The password goes to
// the core once, inside `environment.auth`, which keeps it in the platform's credential store;
// the shell never stores it and never shows one back. What it shows instead is whether a
// credential is stored and for which user (`getBasicAuthStatus`, RB-094-29), and the texts name
// the store the core said it uses (`store`, RB-099-05).

import { ref, type InjectionKey } from 'vue';
import type { CoreToShell, CredentialStoreKind, EnvironmentAuthStatus, ShellToCore } from '../ipc/protocol';

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

/** Asks the core which environments have a stored credential, and for whom (RB-094-29). */
export type AuthStatusQuery = () => Promise<EnvironmentAuthStatus[]>;

/** The shell's query, provided by `bootstrap.ts`; the bar takes it, or its `query` prop. */
export const AUTH_STATUS_QUERY_KEY: InjectionKey<AuthStatusQuery> = Symbol('authStatusQuery');

/** The stored user by environment id; `null` for an environment without a credential. */
export type StoredUsers = Record<string, string | null>;

/** The users of `statuses`, by environment. */
export function storedUsers(statuses: EnvironmentAuthStatus[]): StoredUsers {
  return Object.fromEntries(
    statuses.map((status) => [status.environment, status.stored ? (status.user ?? '') : null]),
  );
}

/** The users of a pushed `basicAuthStatus`, noting its store; `null` for any other message. */
export function usersOf(message: CoreToShell): StoredUsers | null {
  return message.type === 'basicAuthStatus' ? storedUsers(statusesOf(message)) : null;
}

/** The store the core named last on this page; `undefined` until it did. */
const credentialStore = ref<CredentialStoreKind | undefined>(undefined);

/** The statuses of a `basicAuthStatus` answer, noting its store (an older core names none). */
export function statusesOf(message: Extract<CoreToShell, { type: 'basicAuthStatus' }>): EnvironmentAuthStatus[] {
  if (message.store !== undefined) {
    credentialStore.value = message.store;
  }
  return message.environments;
}

/** The i18n key of the name of the store `kind`; the system's store when none or unknown. */
export function storeNameKey(kind: CredentialStoreKind | undefined): string {
  switch (kind) {
    case 'windowsCredentialManager':
      return 'auth.store.windowsCredentialManager';
    case 'macosKeychain':
      return 'auth.store.macosKeychain';
    case 'secretService':
      return 'auth.store.secretService';
    default:
      return 'auth.store.system';
  }
}

/** The `{store}` parameter of the texts that say where the credentials are kept, translated by
 * `t`; reactive, so a text follows when the core names its store. */
export function storeParams(t: (key: string) => string): { store: string } {
  return { store: t(storeNameKey(credentialStore.value)) };
}

/** What a row says about `environment`'s credential, as an i18n key and its parameters; `null`
 * while the core has not said (an older core, a failed query). */
export function storedText(
  users: StoredUsers | null,
  environment: string,
): { key: string; params: Record<string, string> } | null {
  if (users === null) {
    return null;
  }
  const user = users[environment] ?? null;
  return user === null ? { key: 'auth.status.none', params: {} } : { key: 'auth.status.stored', params: { user } };
}

/** Whether Remove has something to remove: a stored credential, or not known either way. */
export function canRemove(users: StoredUsers | null, environment: string): boolean {
  return users === null || (users[environment] ?? null) !== null;
}
