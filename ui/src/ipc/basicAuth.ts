// Basic auth per environment (RB-094-29), mirror of rb-ipc `basic_auth.rs`; kept beside
// `protocol.ts`, which re-exports it. Whether a credential is stored for an environment and for
// which user - never the password, which stays in the platform's credential store. The answer to
// `getBasicAuthStatus`, and pushed after `environment.auth` stored or removed one.

/** One environment's stored credential: the user, never the password. */
export interface EnvironmentAuthStatus {
  /** The environment id. */
  environment: string;
  /** Whether a credential is stored for the environment's address in this project. */
  stored: boolean;
  /** The stored credential's user name; absent when none is stored. */
  user?: string;
}

/** Answered with `basicAuthStatus`. */
export interface BasicAuthShellMessage {
  type: 'getBasicAuthStatus';
}

/** Every environment of the project, in the config's order. */
export interface BasicAuthCoreMessage {
  type: 'basicAuthStatus';
  environments: EnvironmentAuthStatus[];
}
