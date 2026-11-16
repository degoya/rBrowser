// The extensions' messages, mirror of rb-ipc's: the Web Store prompt (RB-080-23) and the list
// of the installed Web Store extensions (RB-080-30); kept beside `protocol.ts`, which folds
// them into its unions and re-exports the types. The core sends `{ type: 'extensionPrompt',
// ...prompt }` before an MCP install, an update or an MCP removal; the shell answers
// `extensionDecision` with the id.

/** rb-ipc `ExtensionPromptKind`: an MCP client's install or removal, or a newer version in the store. */
export type ExtensionPromptKind = 'install' | 'update' | 'remove';

/** rb-ipc `ExtensionPrompt`: the verified name and version that wait for the person's word. */
export interface ExtensionPrompt {
  promptId: number;
  kind: ExtensionPromptKind;
  id: string;
  name: string;
  version: string;
  /** The version installed now (an update). */
  currentVersion?: string;
}

/** rb-ipc `WebStoreExtension` (RB-080-30): one installed Web Store extension of the settings' list. */
export interface WebStoreExtension {
  id: string;
  name: string;
  version: string;
  /** Whether this start loaded it: a removal shows at the next start. */
  loaded: boolean;
}

/** The shell's requests of the extensions: the prompt's answer (`ack`) and the list (`webStoreExtensions`). */
export type ExtensionRequest =
  | { type: 'extensionDecision'; promptId: number; accepted: boolean }
  | { type: 'listWebStoreExtensions' };

/** The core's extension messages: a prompt waiting for `extensionDecision`, the list's answer. */
export type ExtensionEvent =
  | ({ type: 'extensionPrompt' } & ExtensionPrompt)
  | { type: 'webStoreExtensions'; items: WebStoreExtension[] };
