// The MCP setup's clients and what the page makes of a plan (RB-091-09): the order the owner
// named them in, which files a click writes, and the line "Check connection" shows. The files
// and lines themselves are the core's (`crates/rbrowser/src/mcp/clients/`).

import type { McpClient, McpClientPlan, McpFilePlan, McpSetupAnswer } from '../ipc/protocol';

export const MCP_CLIENTS: readonly McpClient[] = ['claudeCode', 'vscode', 'claudeDesktop', 'cursor', 'codex', 'pi'];

/** What the person ticked: the project file, the user file (their confirmation). */
export interface McpChoice {
  project: boolean;
  user: boolean;
}

/** Whether writing `file` changes it: created, added to or its entry replaced. */
export function canWrite(file: McpFilePlan | undefined): boolean {
  return file !== undefined && (file.status === 'create' || file.status === 'add' || file.status === 'replace');
}

/** The project file is ticked when it can be written; the user file never by itself. */
export function defaultChoice(plan: McpClientPlan | undefined): McpChoice {
  return { project: canWrite(plan?.project), user: false };
}

/** How many files a click writes for `plan` with `choice`. */
export function writeCount(plan: McpClientPlan | undefined, choice: McpChoice): number {
  if (plan === undefined) return 0;
  return (choice.project && canWrite(plan.project) ? 1 : 0) + (choice.user && canWrite(plan.userFile) ? 1 : 0);
}

export interface Line {
  key: string;
  args?: Record<string, string | number>;
}

/** The connection check in one line: off, starting, waiting on its port, or reached. */
export function checkLine(check: Extract<McpSetupAnswer, { op: 'check' }>): Line {
  if (!check.enabled) return { key: 'mcp.setup.check.off' };
  if (!check.running || check.port === undefined) return { key: 'mcp.setup.check.starting' };
  if (check.lastCallSecs === undefined) return { key: 'mcp.setup.check.waiting', args: { port: check.port } };
  return { key: 'mcp.setup.check.connected', args: { seconds: check.lastCallSecs, calls: check.calls } };
}
