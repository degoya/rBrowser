// Basic auth per environment (RB-094-29), mirror of rb-ipc `basic_auth.rs`; kept beside
// `protocol.ts`, which re-exports it. Whether a credential is stored for an environment and for
// which user - never the password, which stays in the platform's credential store. The answer to
// `getBasicAuthStatus`, and pushed after `environment.auth` stored or removed one; with it, which
// store this platform uses, so the texts name it (RB-099-05).

/** One environment's stored credential: the user, never the password. */
export interface EnvironmentAuthStatus {
  /** The environment id. */
  environment: string;
  /** Whether a credential is stored for the environment's address in this project. */
  stored: boolean;
  /** The stored credential's user name; absent when none is stored. */
  user?: string;
}

/** Where this platform keeps the credentials: Windows, macOS, Linux, or none (RB-099-05). */
export type CredentialStoreKind = 'windowsCredentialManager' | 'macosKeychain' | 'secretService' | 'none';

/** Answered with `basicAuthStatus`. */
export interface BasicAuthShellMessage {
  type: 'getBasicAuthStatus';
}

/** Every environment of the project, in the config's order. */
export interface BasicAuthCoreMessage {
  type: 'basicAuthStatus';
  environments: EnvironmentAuthStatus[];
  /** The store the credentials are kept in; absent from a core before RB-099-05. */
  store?: CredentialStoreKind;
}
