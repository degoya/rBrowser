// The setup wizard's messages (RB-091-05), part of the shell UI <-> core protocol
// (`protocol.ts` re-exports them; rb-ipc `setup.rs`). `info` and `close` are answered at once;
// `preview`, `write` and `basics` with `ack`, their result follows as a `setup` event carrying the
// request's `requestId` - the core runs them in order, the page takes only its newest answer.

/** Where the project step's breakpoints come from. */
export type SetupTemplate = 'analyzer' | 'default' | 'devices';
/** Where a proposal's breakpoints came from. */
export type SetupSource = 'analyzer' | 'default' | 'devices';
export type SetupLayout = 'grid' | 'canvas';
export type SetupStep = 'preview' | 'write' | 'basics';
/** Why the analyser had no proposal (RB-091-08); the text is `firstrun.project.reason.<code>`. */
export type SetupReasonCode =
  | 'no_page'
  | 'unreachable'
  | 'timeout'
  | 'no_css'
  | 'plugin_missing'
  | 'plugin_failed'
  | 'bad_output'
  | 'nothing_found'
  | 'invalid';
export interface SetupReason {
  code: SetupReasonCode;
  params?: Record<string, string>;
}

export type SetupRequest =
  | { op: 'info' }
  | { op: 'preview'; requestId: number; root: string; url?: string; template: SetupTemplate; devices?: string[] }
  | { op: 'write'; requestId: number; proposalId: number }
  | { op: 'basics'; requestId: number; layout?: SetupLayout; sync?: boolean }
  | { op: 'close' };

export interface SetupProject {
  root: string;
  hasConfig: boolean;
}

/** A JSON value as the core sends it: the config of a proposal. */
export type SetupJson = string | number | boolean | null | SetupJson[] | { [key: string]: SetupJson };

export type SetupAnswer =
  | { op: 'info'; enabled: boolean; project?: SetupProject; written?: string }
  | {
      op: 'proposal';
      requestId: number;
      proposalId: number;
      root: string;
      config: { [key: string]: SetupJson };
      source: SetupSource;
      /** The stylesheets the analyser read, when its proposal is the source. */
      sheets?: number;
      /** Why the analyser had none, when it was asked and the source is `default`. */
      reason?: SetupReason;
    }
  | { op: 'written'; requestId: number; root: string; path: string; open: boolean }
  | { op: 'applied'; requestId: number; root: string }
  | { op: 'failed'; requestId: number; step: SetupStep; messageKey: string; args?: Record<string, string> };

/** The shell's message. */
export type SetupShellMessage = { type: 'setup'; request: SetupRequest };
/** The core's answer or event. */
export type SetupCoreMessage = { type: 'setup'; answer: SetupAnswer };
