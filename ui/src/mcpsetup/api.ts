// What the MCP setup asks of the core (RB-091-09), for the setup wizard's step and Settings >
// Automation > MCP alike: the plan, the write and the connection check over `mcpSetup` (each
// acknowledged; the `mcpSetup` event with the request's number is its answer), and the two user
// settings the person's click writes - `mcp.enabled` (never over MCP) and `mcp.client`.

import type { IpcClient } from '../ipc/client';
import type { McpClient, McpSetupAnswer, McpSetupRequest } from '../ipc/protocol';

type Answer<Op extends McpSetupAnswer['op']> = Extract<McpSetupAnswer, { op: Op }>;
type Failed = Answer<'failed'>;
/** Each request without its number, which this file gives. */
type WithoutId<T> = T extends unknown ? Omit<T, 'requestId'> : never;
type Unnumbered = WithoutId<McpSetupRequest>;

export const ENABLED_KEY = 'mcp.enabled';
export const CLIENT_KEY = 'mcp.client';

/** What the person ticked for a client. */
export interface WriteChoice {
  client: McpClient;
  root?: string | null;
  project: boolean;
  user: boolean;
}

export interface McpSetupApi {
  /** Every client's two ways for the project at `root` (else the open project). */
  plan(root?: string | null): Promise<Answer<'plan'> | Failed>;
  /** Writes what the person ticked; a user file only with `user` - their confirmation. */
  write(choice: WriteChoice): Promise<Answer<'written'> | Failed>;
  check(): Promise<Answer<'check'> | Failed>;
  /** Switches the MCP server on; `false` when the setting was refused. */
  enable(): Promise<boolean>;
  setClient(client: McpClient): Promise<void>;
}

/** The numbers of every page's requests, so two panels never take each other's answers. */
let nextId = 0;

export function createMcpSetupApi(client: IpcClient): McpSetupApi {
  function untilEvent<Op extends McpSetupAnswer['op']>(request: Unnumbered): Promise<Answer<Op> | Failed> {
    nextId += 1;
    const requestId = nextId;
    const numbered = { ...request, requestId } as McpSetupRequest;
    return new Promise((resolve, reject) => {
      const stop = client.on((message) => {
        if (message.type !== 'mcpSetup' || message.answer.requestId !== requestId) return;
        stop();
        resolve(message.answer as Answer<Op> | Failed);
      });
      client.request({ type: 'mcpSetup', request: numbered }).catch((error: unknown) => {
        stop();
        reject(error instanceof Error ? error : new Error(String(error)));
      });
    });
  }

  const withRoot = (root: string | null | undefined) => (root === null || root === undefined ? {} : { root });

  return {
    plan: (root) => untilEvent<'plan'>({ op: 'plan', ...withRoot(root) }),
    write: ({ client: id, root, project, user }) => untilEvent<'written'>({ op: 'write', client: id, ...withRoot(root), project, user }),
    check: () => untilEvent<'check'>({ op: 'check' }),
    async enable() {
      const answer = await client.request({ type: 'setSetting', key: ENABLED_KEY, value: true, scope: 'user' });
      return answer.type !== 'settingRejected';
    },
    async setClient(id) {
      await client.request({ type: 'setSetting', key: CLIENT_KEY, value: id, scope: 'user' });
    },
  };
}
