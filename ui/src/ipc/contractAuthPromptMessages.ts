// The sign-in dialog's messages of the contract test's typed literals (`contract.test.ts`,
// RB-099-04), in a file of its own so the core list stays under the file-length limit.

import type { CoreToShell, ShellToCore } from './protocol';

export const authPromptShellMessages = [
  { type: 'authPromptReply', id: 3, signIn: { user: 'ada', password: 'contract-pw-1', save: true } },
] satisfies ShellToCore[];

export const authPromptCoreMessages = [
  {
    type: 'authPrompt',
    id: 3,
    origin: 'https://dev.example.test:443',
    realm: 'Staging',
    proxy: false,
    views: [1, 2],
    environment: 'development',
    refused: false,
    insecure: false,
  },
  { type: 'authPromptClosed', id: 3 },
] satisfies CoreToShell[];
