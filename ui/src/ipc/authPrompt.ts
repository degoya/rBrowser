// rBrowser's own sign-in dialog (RB-099-04), mirror of rb-ipc `auth_prompt.rs`; kept beside
// `protocol.ts`, which re-exports it. A challenge the stored credentials do not answer is asked
// in the shell's bar: the core asks (`authPrompt`, again with the same id when more views wait)
// and withdraws (`authPromptClosed`); the shell answers once (`authPromptReply`) with user and
// password, or without them for Cancel. The environment a credential is saved for is the core's.

import type { ViewId } from './protocol';

/** One open sign-in question: one per origin, realm and proxy. */
export interface AuthPrompt {
  id: number;
  /** `scheme://host:port` of the server, `host:port` of a proxy. */
  origin: string;
  /** The realm the server names; absent when none. */
  realm?: string;
  proxy: boolean;
  /** The views waiting, in order. */
  views: ViewId[];
  /** The environment whose address this is, when "Save for environment" may be offered. */
  environment?: string;
  /** The server refused the credential sent before. */
  refused: boolean;
  /** The answer would cross the network unencrypted (plain http to another machine). */
  insecure: boolean;
}

/** The user and password typed, and whether to store them for the question's environment. */
export interface AuthPromptSignIn {
  user: string;
  password: string;
  save: boolean;
}

/** The person's answer; without `signIn` it cancels. Answered with `ack`. */
export interface AuthPromptShellMessage {
  type: 'authPromptReply';
  id: number;
  signIn?: AuthPromptSignIn;
}

/** The core asks, or names more views waiting. */
export interface AuthPromptCoreMessage extends AuthPrompt {
  type: 'authPrompt';
}

/** The question is withdrawn: its views closed, or it waited too long. */
export interface AuthPromptClosedMessage {
  type: 'authPromptClosed';
  id: number;
}

export type AuthPromptEvent = AuthPromptCoreMessage | AuthPromptClosedMessage;
