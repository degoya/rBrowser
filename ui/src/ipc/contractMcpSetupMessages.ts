// The MCP setup's messages of the contract test's typed literals (`contract.test.ts`,
// RB-091-09), in a file of its own so the core list stays under the file-length limit.

import type { CoreToShell, ShellToCore } from './protocol';

export const mcpSetupShellMessages = [
  {
    type: 'mcpSetup',
    request: { op: 'write', requestId: 4, client: 'cursor', root: 'C:/work/shop', project: true, user: false },
  },
] satisfies ShellToCore[];

export const mcpSetupCoreMessages = [
  {
    type: 'mcpSetup',
    answer: {
      op: 'plan',
      requestId: 1,
      enabled: false,
      root: 'C:/work/shop',
      clients: [
        {
          client: 'cursor',
          project: {
            path: '.cursor/mcp.json',
            status: 'create',
            entry: '{\n  "type": "stdio",\n  "command": "C:\\\\Tools\\\\rBrowser\\\\rbrowser.exe",\n  "args": [\n    "mcp"\n  ]\n}',
            kept: 0,
          },
          userFile: {
            path: 'C:\\Users\\me\\.cursor\\mcp.json',
            status: 'add',
            entry: '{}',
            kept: 2,
          },
          note: 'mcp.setup.note.cursor',
        },
      ],
    },
  },
] satisfies CoreToShell[];
