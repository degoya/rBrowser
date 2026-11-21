// The MCP setup's messages (RB-091-09), part of the shell UI <-> core protocol (`protocol.ts`
// re-exports them; rb-ipc `mcp_setup.rs`). From the pages window only: the setup wizard's step
// "Connect an AI agent" and Settings > Automation > MCP. Every request is answered with `ack`;
// its result follows as a `mcpSetup` event carrying the request's `requestId`.

/** The clients the setup knows, as `mcp.client` names them. */
export type McpClient = 'claudeCode' | 'vscode' | 'claudeDesktop' | 'cursor' | 'codex' | 'pi';
/** What a write does to a file. */
export type McpFileStatus = 'create' | 'add' | 'unchanged' | 'replace' | 'unreadable';
/** Where a command line runs. */
export type McpShell = 'any' | 'windows' | 'wsl';

export type McpSetupRequest =
  | { op: 'plan'; requestId: number; root?: string }
  | { op: 'write'; requestId: number; client: McpClient; root?: string; project: boolean; user: boolean }
  | { op: 'check'; requestId: number };

export interface McpFilePlan {
  /** Relative to the project for a project file, absolute for a user file. */
  path: string;
  status: McpFileStatus;
  /** The `rbrowser` entry as it is written. */
  entry: string;
  /** The other servers in the file, which stay as they are. */
  kept: number;
}

export interface McpCommandLine {
  shell: McpShell;
  line: string;
}

export interface McpClientPlan {
  client: McpClient;
  project?: McpFilePlan;
  /** Why there is no project file (an i18n key). */
  projectReason?: string;
  userFile?: McpFilePlan;
  commands?: McpCommandLine[];
  /** What the person should know about the client (an i18n key). */
  note?: string;
}

export interface McpWrittenFile {
  path: string;
  status: McpFileStatus;
  backup?: string;
}

export type McpSetupAnswer =
  | { op: 'plan'; requestId: number; enabled: boolean; root?: string; clients: McpClientPlan[] }
  | { op: 'written'; requestId: number; files: McpWrittenFile[] }
  | {
      op: 'check';
      requestId: number;
      enabled: boolean;
      running: boolean;
      port?: number;
      lastCallSecs?: number;
      calls: number;
    }
  | { op: 'failed'; requestId: number; messageKey: string; args?: Record<string, string> };

/** The shell's message. */
export type McpSetupShellMessage = { type: 'mcpSetup'; request: McpSetupRequest };
/** The core's event. */
export type McpSetupCoreMessage = { type: 'mcpSetup'; answer: McpSetupAnswer };
