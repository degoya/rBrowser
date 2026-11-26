// The Basic auth status messages of the contract test's typed literals (`contract.test.ts`,
// RB-094-29), in a file of its own so the core list stays under the file-length limit.

import type { CoreToShell, ShellToCore } from './protocol';

export const basicAuthShellMessages = [{ type: 'getBasicAuthStatus' }] satisfies ShellToCore[];

export const basicAuthCoreMessages = [
  {
    type: 'basicAuthStatus',
    environments: [
      { environment: 'local', stored: false },
      { environment: 'staging', stored: true, user: 'ada' },
    ],
    store: 'windowsCredentialManager',
  },
] satisfies CoreToShell[];
